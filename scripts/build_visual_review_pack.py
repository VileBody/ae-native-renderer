#!/usr/bin/env python3
"""Build contact-sheet review artifacts for conformance outputs."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from PIL import Image, ImageDraw, ImageFont


TUNING_METRIC = "rgb_straight_source_over_ae_background"


@dataclass(frozen=True)
class FrameMetric:
    frame: int
    ae: Path
    native: Path
    diff: Path
    visible_rgb: float | None
    alpha: float | None
    max_abs_diff: int | None


def parse_args() -> argparse.Namespace:
    ap = argparse.ArgumentParser(
        description="Create manual visual review sheets from conformance outputs."
    )
    ap.add_argument("--after", type=Path, required=True, help="New conformance output dir.")
    ap.add_argument("--before", type=Path, help="Optional previous conformance output dir.")
    ap.add_argument("--out", type=Path, required=True, help="Review pack output dir.")
    ap.add_argument(
        "--case",
        dest="cases",
        action="append",
        help="Case id to include. Defaults to every metrics-bearing case in --after.",
    )
    ap.add_argument("--thumb", type=int, default=190, help="Thumbnail box size in pixels.")
    return ap.parse_args()


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def available_cases(root: Path) -> list[str]:
    return sorted(p.name for p in root.iterdir() if (p / "metrics.json").exists())


def frame_metric(row: dict[str, Any]) -> tuple[float | None, float | None]:
    diagnostics = row.get("alpha_policy_diagnostics") or {}
    mean_abs = diagnostics.get("mean_abs_diff") or {}
    visible = mean_abs.get(TUNING_METRIC)
    alpha = mean_abs.get("alpha")
    return (
        float(visible) if visible is not None else None,
        float(alpha) if alpha is not None else None,
    )


def case_summary(metrics: dict[str, Any]) -> float | None:
    summary = metrics.get("summary")
    if isinstance(summary, (int, float)):
        return float(summary)
    if not isinstance(summary, dict):
        return None
    diagnostics = summary.get("alpha_policy_diagnostics") or {}
    mean_abs = diagnostics.get("mean_abs_diff") or {}
    visible = mean_abs.get(TUNING_METRIC)
    return float(visible) if visible is not None else None


def read_case(root: Path, case_id: str) -> tuple[dict[str, Any], list[FrameMetric]]:
    metrics_path = root / case_id / "metrics.json"
    metrics = load_json(metrics_path)
    frames: list[FrameMetric] = []
    for row in metrics.get("frames", []):
        visible, alpha = frame_metric(row)
        frames.append(
            FrameMetric(
                frame=int(row["frame"]),
                ae=Path(row["ae"]),
                native=Path(row["native"]),
                diff=Path(row["diff"]),
                visible_rgb=visible,
                alpha=alpha,
                max_abs_diff=(
                    int(row["max_abs_diff"]) if row.get("max_abs_diff") is not None else None
                ),
            )
        )
    frames.sort(key=lambda f: f.frame)
    return metrics, frames


def load_font(size: int) -> ImageFont.ImageFont:
    candidates = [
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/Library/Fonts/Arial.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ]
    for candidate in candidates:
        path = Path(candidate)
        if path.exists():
            return ImageFont.truetype(str(path), size=size)
    return ImageFont.load_default()


def checkerboard(size: tuple[int, int], block: int = 16) -> Image.Image:
    w, h = size
    out = Image.new("RGBA", size, (38, 42, 48, 255))
    draw = ImageDraw.Draw(out)
    for y in range(0, h, block):
        for x in range(0, w, block):
            if ((x // block) + (y // block)) % 2:
                draw.rectangle((x, y, x + block - 1, y + block - 1), fill=(64, 68, 76, 255))
    return out


def read_display_image(path: Path, thumb: int) -> Image.Image:
    image = Image.open(path).convert("RGBA")
    base = checkerboard(image.size)
    base.alpha_composite(image)
    base.thumbnail((thumb, thumb), Image.Resampling.LANCZOS)
    tile = Image.new("RGBA", (thumb, thumb), (20, 22, 26, 255))
    x = (thumb - base.width) // 2
    y = (thumb - base.height) // 2
    tile.alpha_composite(base, (x, y))
    return tile.convert("RGB")


def text(draw: ImageDraw.ImageDraw, xy: tuple[int, int], value: str, font: ImageFont.ImageFont) -> None:
    draw.text(xy, value, fill=(236, 238, 242), font=font)


def label_for(metric: FrameMetric) -> str:
    parts = [f"f={metric.frame:05d}"]
    if metric.visible_rgb is not None:
        parts.append(f"vis={metric.visible_rgb:.3f}")
    if metric.alpha is not None:
        parts.append(f"a={metric.alpha:.3f}")
    if metric.max_abs_diff is not None:
        parts.append(f"max={metric.max_abs_diff}")
    return " ".join(parts)


def draw_contact_sheet(
    output: Path,
    title: str,
    headers: list[str],
    rows: list[tuple[str, list[Path]]],
    thumb: int,
) -> None:
    font = load_font(14)
    title_font = load_font(18)
    pad = 12
    label_h = 30
    header_h = 64
    row_h = thumb + label_h + pad
    col_w = thumb + pad
    w = pad + len(headers) * col_w
    h = header_h + len(rows) * row_h + pad
    sheet = Image.new("RGB", (w, h), (14, 16, 20))
    draw = ImageDraw.Draw(sheet)
    text(draw, (pad, 12), title, title_font)
    for idx, header in enumerate(headers):
        text(draw, (pad + idx * col_w, 40), header, font)
    for row_idx, (row_label, paths) in enumerate(rows):
        y = header_h + row_idx * row_h
        text(draw, (pad, y), row_label, font)
        for col_idx, path in enumerate(paths):
            x = pad + col_idx * col_w
            image = read_display_image(path, thumb)
            sheet.paste(image, (x, y + label_h))
    output.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(output)


def build_after_sheet(out_dir: Path, case_id: str, metrics: dict[str, Any], frames: list[FrameMetric], thumb: int) -> dict[str, Any]:
    rows = [(label_for(f), [f.ae, f.native, f.diff]) for f in frames]
    output = out_dir / f"{case_id}_contact_sheet.png"
    summary = case_summary(metrics)
    title = f"{case_id} after"
    if summary is not None:
        title += f" summary={summary:.4f}"
    draw_contact_sheet(output, title, ["AE", "Native", "Diff"], rows, thumb)
    return {
        "case": case_id,
        "type": "after_contact_sheet",
        "path": str(output),
        "summary_visible_rgb": summary,
        "frames": [f.frame for f in frames],
    }


def build_before_after_sheet(
    out_dir: Path,
    case_id: str,
    before_metrics: dict[str, Any],
    before_frames: list[FrameMetric],
    after_metrics: dict[str, Any],
    after_frames: list[FrameMetric],
    thumb: int,
) -> dict[str, Any]:
    before_by_frame = {f.frame: f for f in before_frames}
    after_by_frame = {f.frame: f for f in after_frames}
    frames = sorted(set(before_by_frame) & set(after_by_frame))
    rows = []
    for frame in frames:
        before = before_by_frame[frame]
        after = after_by_frame[frame]
        label = f"f={frame:05d}"
        if before.visible_rgb is not None and after.visible_rgb is not None:
            label += f" before={before.visible_rgb:.3f} after={after.visible_rgb:.3f}"
        rows.append(
            (
                label,
                [after.ae, before.native, before.diff, after.native, after.diff],
            )
        )
    before_summary = case_summary(before_metrics)
    after_summary = case_summary(after_metrics)
    output = out_dir / f"{case_id}_before_after.png"
    title = f"{case_id} before/after"
    if before_summary is not None and after_summary is not None:
        title += f" {before_summary:.4f}->{after_summary:.4f}"
    draw_contact_sheet(
        output,
        title,
        ["AE", "Before Native", "Before Diff", "After Native", "After Diff"],
        rows,
        thumb,
    )
    return {
        "case": case_id,
        "type": "before_after_contact_sheet",
        "path": str(output),
        "before_summary_visible_rgb": before_summary,
        "after_summary_visible_rgb": after_summary,
        "frames": frames,
    }


def write_readme(out_dir: Path, manifest: dict[str, Any]) -> None:
    lines = [
        "# Visual Review Pack",
        "",
        f"After: `{manifest['after']}`",
    ]
    if manifest.get("before"):
        lines.append(f"Before: `{manifest['before']}`")
    lines.extend(["", "## Sheets", ""])
    for artifact in manifest["artifacts"]:
        lines.append(f"- `{Path(artifact['path']).name}`: {artifact['type']} for `{artifact['case']}`")
    lines.extend(
        [
            "",
            "## Metric",
            "",
            f"Primary visible metric: `{TUNING_METRIC}`.",
            "Use these sheets for manual AE/native/diff inspection before promoting a module.",
            "",
        ]
    )
    (out_dir / "README.md").write_text("\n".join(lines), encoding="utf-8")


def main() -> None:
    args = parse_args()
    cases = args.cases or available_cases(args.after)
    args.out.mkdir(parents=True, exist_ok=True)
    manifest: dict[str, Any] = {
        "schema": "visual_review_pack.v1",
        "after": str(args.after),
        "before": str(args.before) if args.before else None,
        "metric": TUNING_METRIC,
        "artifacts": [],
    }
    for case_id in cases:
        after_metrics_path = args.after / case_id / "metrics.json"
        if not after_metrics_path.exists():
            continue
        after_metrics, after_frames = read_case(args.after, case_id)
        manifest["artifacts"].append(
            build_after_sheet(args.out, case_id, after_metrics, after_frames, args.thumb)
        )
        if args.before and (args.before / case_id / "metrics.json").exists():
            before_metrics, before_frames = read_case(args.before, case_id)
            manifest["artifacts"].append(
                build_before_after_sheet(
                    args.out,
                    case_id,
                    before_metrics,
                    before_frames,
                    after_metrics,
                    after_frames,
                    args.thumb,
                )
            )
    (args.out / "manifest.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=True) + "\n",
        encoding="utf-8",
    )
    write_readme(args.out, manifest)
    print(args.out)


if __name__ == "__main__":
    main()
