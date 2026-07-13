#!/usr/bin/env python3
"""Compare Rust and After Effects PNGs at exactly the same frame numbers."""

from __future__ import annotations

import argparse
import json
import math
import re
import string
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Sequence

import numpy as np
from PIL import Image, UnidentifiedImageError


SCHEMA = "ae-native-renderer.frame-locked-comparison.v1"
CHANNEL_NAMES = ("r", "g", "b", "a")
FRAME_NUMBER_RE = re.compile(r"(\d+)$")
INTEGER_RE = re.compile(r"^[+-]?\d+$")
RANGE_RE = re.compile(r"^([+-]?\d+)\s*-\s*([+-]?\d+)$")
MAX_FRAME_COUNT = 100_000
STAT_CHUNK_PIXELS = 262_144


class ComparisonError(RuntimeError):
    """An input cannot be compared without changing the requested contract."""


@dataclass(frozen=True)
class FramePair:
    frame: int
    rust_path: Path
    ae_path: Path
    size: tuple[int, int]
    rust_mode: str
    ae_mode: str


@dataclass(frozen=True)
class MetricTotals:
    absolute_sum: float
    squared_sum: float
    sample_count: int
    changed_pixels: int
    pixel_count: int
    max_abs_diff: int
    ssim: float
    identical: bool


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Compare Rust and AE PNG sequences at identical frame numbers. "
            "No temporal offset, nearest-frame lookup, or resizing is performed."
        )
    )
    parser.add_argument(
        "--rust-dir",
        "--rust-frames-dir",
        dest="rust_dir",
        type=Path,
        required=True,
        help="Directory containing Rust PNG frames.",
    )
    parser.add_argument(
        "--ae-dir",
        "--ae-frames-dir",
        dest="ae_dir",
        type=Path,
        required=True,
        help="Directory containing After Effects PNG frames.",
    )
    parser.add_argument(
        "--frames",
        nargs="+",
        required=True,
        metavar="SPEC",
        help="Inclusive ranges and/or numbers, e.g. 0-12,20,24 or 0 4 8-10.",
    )
    parser.add_argument(
        "--out-dir",
        "--out",
        dest="out_dir",
        type=Path,
        required=True,
        help="Directory for comparison PNGs and metrics.json.",
    )
    parser.add_argument(
        "--rust-pattern",
        help="Optional relative format pattern, e.g. frame_{frame:06d}.png.",
    )
    parser.add_argument(
        "--ae-pattern",
        help="Optional relative format pattern, e.g. CASE_{frame:05d}.png.",
    )
    return parser


def parse_frame_specs(specs: Sequence[str]) -> list[int]:
    frames: set[int] = set()
    for spec in specs:
        for raw_part in spec.split(","):
            part = raw_part.strip()
            if not part:
                raise ComparisonError(f"invalid empty item in frame specification {spec!r}")
            if INTEGER_RE.fullmatch(part):
                frames.add(int(part))
                continue
            match = RANGE_RE.fullmatch(part)
            if match is None:
                raise ComparisonError(
                    f"invalid frame item {part!r}; use integers or inclusive START-END ranges"
                )
            start, end = (int(value) for value in match.groups())
            if end < start:
                raise ComparisonError(
                    f"descending frame range {part!r} is not allowed; use {end}-{start}"
                )
            if end - start + 1 > MAX_FRAME_COUNT:
                raise ComparisonError(
                    f"frame range {part!r} exceeds the {MAX_FRAME_COUNT} frame limit"
                )
            frames.update(range(start, end + 1))
            if len(frames) > MAX_FRAME_COUNT:
                raise ComparisonError(
                    f"frame selection exceeds the {MAX_FRAME_COUNT} frame limit"
                )
    if not frames:
        raise ComparisonError("at least one frame number is required")
    return sorted(frames)


def validate_directory(path: Path, label: str) -> Path:
    resolved = path.expanduser().resolve()
    if not resolved.exists():
        raise ComparisonError(f"{label} directory does not exist: {resolved}")
    if not resolved.is_dir():
        raise ComparisonError(f"{label} path is not a directory: {resolved}")
    return resolved


def validate_pattern(pattern: str, label: str) -> None:
    try:
        fields = [
            field for _, field, _, _ in string.Formatter().parse(pattern) if field
        ]
        if fields != ["frame"]:
            raise ComparisonError(
                f"{label} pattern must contain exactly one {{frame}} field: {pattern!r}"
            )
        rendered = pattern.format(frame=0)
    except (IndexError, KeyError, ValueError) as exc:
        raise ComparisonError(f"invalid {label} pattern {pattern!r}: {exc}") from exc
    if Path(rendered).is_absolute():
        raise ComparisonError(f"{label} pattern must be relative to its frames directory")


def patterned_path(directory: Path, pattern: str, frame: int, label: str) -> Path:
    path = (directory / pattern.format(frame=frame)).resolve()
    try:
        path.relative_to(directory)
    except ValueError as exc:
        raise ComparisonError(
            f"{label} pattern resolves outside its frames directory for frame {frame}: {path}"
        ) from exc
    return path


def index_numbered_pngs(directory: Path) -> dict[int, list[Path]]:
    indexed: dict[int, list[Path]] = {}
    for path in sorted(directory.iterdir(), key=lambda item: (item.name.casefold(), item.name)):
        if not path.is_file() or path.suffix.lower() != ".png":
            continue
        match = FRAME_NUMBER_RE.search(path.stem)
        if match is None:
            continue
        indexed.setdefault(int(match.group(1)), []).append(path.resolve())
    return indexed


def resolve_frame_paths(
    directory: Path,
    frames: Sequence[int],
    pattern: str | None,
    label: str,
) -> dict[int, Path]:
    if pattern is not None:
        validate_pattern(pattern, label)
        resolved: dict[int, Path] = {}
        missing: list[tuple[int, Path]] = []
        for frame in frames:
            path = patterned_path(directory, pattern, frame, label)
            if not path.is_file():
                missing.append((frame, path))
            else:
                resolved[frame] = path
        if missing:
            details = ", ".join(f"{frame} ({path})" for frame, path in missing)
            raise ComparisonError(f"{label} frame(s) missing: {details}")
        return resolved

    indexed = index_numbered_pngs(directory)
    missing = [frame for frame in frames if frame not in indexed]
    ambiguous = {frame: indexed[frame] for frame in frames if len(indexed.get(frame, [])) > 1}
    if missing:
        numbers = ", ".join(str(frame) for frame in missing)
        raise ComparisonError(
            f"{label} frame number(s) not found in {directory}: {numbers}. "
            "Automatic matching uses the final unsigned digit run in each PNG stem; "
            f"use --{label.lower()}-pattern for another naming scheme."
        )
    if ambiguous:
        details = "; ".join(
            f"{frame}: {', '.join(str(path) for path in paths)}"
            for frame, paths in sorted(ambiguous.items())
        )
        raise ComparisonError(
            f"ambiguous {label} frame number(s): {details}. Use an explicit pattern."
        )
    return {frame: indexed[frame][0] for frame in frames}


def inspect_png(path: Path, label: str, frame: int) -> tuple[tuple[int, int], str]:
    try:
        with Image.open(path) as image:
            image_format = image.format
            size = image.size
            mode = image.mode
            image.verify()
    except (OSError, SyntaxError, UnidentifiedImageError, ValueError) as exc:
        raise ComparisonError(f"cannot read {label} frame {frame} as PNG ({path}): {exc}") from exc
    if image_format != "PNG":
        raise ComparisonError(
            f"{label} frame {frame} is not a PNG image despite its path: {path}"
        )
    if size[0] <= 0 or size[1] <= 0:
        raise ComparisonError(f"{label} frame {frame} has invalid dimensions {size}: {path}")
    return size, mode


def build_frame_pairs(
    rust_paths: dict[int, Path], ae_paths: dict[int, Path], frames: Sequence[int]
) -> list[FramePair]:
    pairs: list[FramePair] = []
    for frame in frames:
        rust_size, rust_mode = inspect_png(rust_paths[frame], "Rust", frame)
        ae_size, ae_mode = inspect_png(ae_paths[frame], "AE", frame)
        if rust_size != ae_size:
            raise ComparisonError(
                f"frame {frame} dimensions differ: Rust={rust_size[0]}x{rust_size[1]}, "
                f"AE={ae_size[0]}x{ae_size[1]}; resizing is intentionally disabled"
            )
        pairs.append(
            FramePair(
                frame=frame,
                rust_path=rust_paths[frame],
                ae_path=ae_paths[frame],
                size=rust_size,
                rust_mode=rust_mode,
                ae_mode=ae_mode,
            )
        )
    return pairs


def load_rgba8(path: Path, label: str, frame: int) -> tuple[Image.Image, np.ndarray]:
    try:
        with Image.open(path) as source:
            source.load()
            image = source.convert("RGBA")
    except (OSError, SyntaxError, UnidentifiedImageError, ValueError) as exc:
        raise ComparisonError(f"cannot decode {label} frame {frame} ({path}): {exc}") from exc
    pixels = np.asarray(image, dtype=np.uint8)
    return image, pixels


def stable_float(value: float) -> float:
    rounded = round(float(value), 12)
    return 0.0 if rounded == 0.0 else rounded


def psnr_from_mse(mse: float) -> float | None:
    if mse == 0.0:
        return None
    return stable_float(10.0 * math.log10((255.0 * 255.0) / mse))


def global_ssim_per_channel(
    sum_x: np.ndarray,
    sum_y: np.ndarray,
    sum_x2: np.ndarray,
    sum_y2: np.ndarray,
    sum_xy: np.ndarray,
    pixel_count: int,
    squared_diff_sum: np.ndarray,
) -> np.ndarray:
    if np.all(squared_diff_sum == 0.0):
        return np.ones(4, dtype=np.float64)
    count = float(pixel_count)
    mean_x = sum_x / count
    mean_y = sum_y / count
    variance_x = np.maximum(sum_x2 / count - mean_x * mean_x, 0.0)
    variance_y = np.maximum(sum_y2 / count - mean_y * mean_y, 0.0)
    covariance = sum_xy / count - mean_x * mean_y
    c1 = (0.01 * 255.0) ** 2
    c2 = (0.03 * 255.0) ** 2
    numerator = (2.0 * mean_x * mean_y + c1) * (2.0 * covariance + c2)
    denominator = (mean_x * mean_x + mean_y * mean_y + c1) * (
        variance_x + variance_y + c2
    )
    scores = numerator / denominator
    scores[squared_diff_sum == 0.0] = 1.0
    return np.clip(scores, -1.0, 1.0)


def grouped_metrics(
    channels: Sequence[int],
    absolute_sum: np.ndarray,
    squared_sum: np.ndarray,
    ssim: np.ndarray,
    max_abs: np.ndarray,
    pixel_count: int,
) -> dict[str, Any]:
    sample_count = pixel_count * len(channels)
    mae = float(np.sum(absolute_sum[list(channels)])) / sample_count
    mse = float(np.sum(squared_sum[list(channels)])) / sample_count
    return {
        "mae": stable_float(mae),
        "mse": stable_float(mse),
        "psnr_db": psnr_from_mse(mse),
        "psnr_is_infinite": mse == 0.0,
        "ssim": stable_float(float(np.mean(ssim[list(channels)]))),
        "max_abs_diff": int(np.max(max_abs[list(channels)])),
        "sample_count": sample_count,
    }


def compute_metrics(
    rust: np.ndarray, ae: np.ndarray, diff: np.ndarray
) -> tuple[dict[str, Any], MetricTotals]:
    height, width, channels = rust.shape
    if ae.shape != rust.shape or diff.shape != rust.shape or channels != 4:
        raise ComparisonError("internal RGBA buffer shape mismatch")
    pixel_count = width * height
    rust_flat = rust.reshape(pixel_count, 4)
    ae_flat = ae.reshape(pixel_count, 4)
    sum_x = np.zeros(4, dtype=np.float64)
    sum_y = np.zeros(4, dtype=np.float64)
    sum_x2 = np.zeros(4, dtype=np.float64)
    sum_y2 = np.zeros(4, dtype=np.float64)
    sum_xy = np.zeros(4, dtype=np.float64)
    absolute_sum = np.zeros(4, dtype=np.float64)
    squared_sum = np.zeros(4, dtype=np.float64)

    for start in range(0, pixel_count, STAT_CHUNK_PIXELS):
        end = min(start + STAT_CHUNK_PIXELS, pixel_count)
        x = rust_flat[start:end].astype(np.float64)
        y = ae_flat[start:end].astype(np.float64)
        delta = x - y
        sum_x += np.sum(x, axis=0)
        sum_y += np.sum(y, axis=0)
        sum_x2 += np.sum(x * x, axis=0)
        sum_y2 += np.sum(y * y, axis=0)
        sum_xy += np.sum(x * y, axis=0)
        absolute_sum += np.sum(np.abs(delta), axis=0)
        squared_sum += np.sum(delta * delta, axis=0)

    ssim = global_ssim_per_channel(
        sum_x, sum_y, sum_x2, sum_y2, sum_xy, pixel_count, squared_sum
    )
    diff_flat = diff.reshape(pixel_count, 4)
    max_abs = np.max(diff_flat, axis=0)
    changed_mask = np.any(diff != 0, axis=2)
    changed_pixels = int(np.count_nonzero(changed_mask))
    if changed_pixels:
        ys, xs = np.nonzero(changed_mask)
        bbox: list[int] | None = [
            int(np.min(xs)),
            int(np.min(ys)),
            int(np.max(xs)) + 1,
            int(np.max(ys)) + 1,
        ]
    else:
        bbox = None

    rgba = grouped_metrics(
        (0, 1, 2, 3), absolute_sum, squared_sum, ssim, max_abs, pixel_count
    )
    rgb = grouped_metrics((0, 1, 2), absolute_sum, squared_sum, ssim, max_abs, pixel_count)
    alpha = grouped_metrics((3,), absolute_sum, squared_sum, ssim, max_abs, pixel_count)
    per_channel = {
        name: grouped_metrics(
            (index,), absolute_sum, squared_sum, ssim, max_abs, pixel_count
        )
        for index, name in enumerate(CHANNEL_NAMES)
    }
    metrics = {
        "identical": changed_pixels == 0,
        "bbox": bbox,
        "changed_pixels": changed_pixels,
        "changed_pixel_ratio": stable_float(changed_pixels / pixel_count),
        "pixel_count": pixel_count,
        **rgba,
        "rgb": rgb,
        "alpha": alpha,
        "per_channel": per_channel,
    }
    totals = MetricTotals(
        absolute_sum=float(np.sum(absolute_sum)),
        squared_sum=float(np.sum(squared_sum)),
        sample_count=pixel_count * 4,
        changed_pixels=changed_pixels,
        pixel_count=pixel_count,
        max_abs_diff=int(np.max(max_abs)),
        ssim=float(rgba["ssim"]),
        identical=changed_pixels == 0,
    )
    return metrics, totals


def absolute_diff(rust: np.ndarray, ae: np.ndarray) -> np.ndarray:
    high = np.maximum(rust, ae)
    low = np.minimum(rust, ae)
    return high - low


def diff_visual(diff: np.ndarray) -> Image.Image:
    alpha_diff = diff[:, :, 3:4]
    visible_rgb = np.maximum(diff[:, :, :3], alpha_diff)
    return Image.fromarray(visible_rgb)


def save_png_atomic(image: Image.Image, path: Path) -> None:
    temporary = path.with_name(f".{path.name}.tmp")
    try:
        image.save(temporary, format="PNG", optimize=False, compress_level=9)
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def write_json_atomic(payload: dict[str, Any], path: Path) -> None:
    temporary = path.with_name(f".{path.name}.tmp")
    rendered = json.dumps(
        payload,
        indent=2,
        sort_keys=True,
        ensure_ascii=True,
        allow_nan=False,
    ) + "\n"
    try:
        temporary.write_text(rendered, encoding="utf-8")
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def frame_token(frame: int) -> str:
    prefix = "m" if frame < 0 else ""
    return f"{prefix}{abs(frame):06d}"


def summary_from_totals(totals: Sequence[MetricTotals]) -> dict[str, Any]:
    absolute_sum = sum(item.absolute_sum for item in totals)
    squared_sum = sum(item.squared_sum for item in totals)
    sample_count = sum(item.sample_count for item in totals)
    changed_pixels = sum(item.changed_pixels for item in totals)
    pixel_count = sum(item.pixel_count for item in totals)
    mse = squared_sum / sample_count
    ssim_values = [item.ssim for item in totals]
    return {
        "frame_count": len(totals),
        "identical_frame_count": sum(item.identical for item in totals),
        "changed_frame_count": sum(not item.identical for item in totals),
        "changed_pixels": changed_pixels,
        "changed_pixel_ratio": stable_float(changed_pixels / pixel_count),
        "pixel_count": pixel_count,
        "sample_count": sample_count,
        "mae": stable_float(absolute_sum / sample_count),
        "mse": stable_float(mse),
        "psnr_db": psnr_from_mse(mse),
        "psnr_is_infinite": mse == 0.0,
        "ssim": stable_float(sum(ssim_values) / len(ssim_values)),
        "min_frame_ssim": stable_float(min(ssim_values)),
        "max_abs_diff": max(item.max_abs_diff for item in totals),
    }


def run_comparison(
    rust_dir: Path,
    ae_dir: Path,
    frame_specs: Sequence[str],
    out_dir: Path,
    rust_pattern: str | None = None,
    ae_pattern: str | None = None,
) -> Path:
    frames = parse_frame_specs(frame_specs)
    rust_dir = validate_directory(rust_dir, "Rust")
    ae_dir = validate_directory(ae_dir, "AE")
    out_dir = out_dir.expanduser().resolve()
    if out_dir in {rust_dir, ae_dir}:
        raise ComparisonError("output directory must differ from both input directories")
    if out_dir.exists() and not out_dir.is_dir():
        raise ComparisonError(f"output path exists and is not a directory: {out_dir}")

    rust_paths = resolve_frame_paths(rust_dir, frames, rust_pattern, "Rust")
    ae_paths = resolve_frame_paths(ae_dir, frames, ae_pattern, "AE")
    pairs = build_frame_pairs(rust_paths, ae_paths, frames)
    out_dir.mkdir(parents=True, exist_ok=True)

    frame_reports: list[dict[str, Any]] = []
    totals: list[MetricTotals] = []
    for pair in pairs:
        rust_image, rust_pixels = load_rgba8(pair.rust_path, "Rust", pair.frame)
        ae_image, ae_pixels = load_rgba8(pair.ae_path, "AE", pair.frame)
        diff = absolute_diff(rust_pixels, ae_pixels)
        metrics, frame_totals = compute_metrics(rust_pixels, ae_pixels, diff)

        token = frame_token(pair.frame)
        side_name = f"frame_{token}_rust_left_ae_right.png"
        diff_name = f"frame_{token}_abs_diff.png"
        side_by_side = Image.new("RGBA", (pair.size[0] * 2, pair.size[1]))
        side_by_side.paste(rust_image, (0, 0))
        side_by_side.paste(ae_image, (pair.size[0], 0))
        save_png_atomic(side_by_side, out_dir / side_name)
        save_png_atomic(diff_visual(diff), out_dir / diff_name)

        frame_reports.append(
            {
                "frame": pair.frame,
                "rust_path": str(pair.rust_path),
                "ae_path": str(pair.ae_path),
                "rust_source_mode": pair.rust_mode,
                "ae_source_mode": pair.ae_mode,
                "width": pair.size[0],
                "height": pair.size[1],
                "artifacts": {
                    "side_by_side": side_name,
                    "abs_diff": diff_name,
                },
                "metrics": metrics,
            }
        )
        totals.append(frame_totals)

    report = {
        "schema": SCHEMA,
        "alignment": {
            "mode": "exact_same_frame_number",
            "frame_offset": 0,
            "nearest_frame_matching": False,
            "requested_frames": frames,
        },
        "inputs": {
            "rust_frames_dir": str(rust_dir),
            "ae_frames_dir": str(ae_dir),
            "rust_pattern": rust_pattern,
            "ae_pattern": ae_pattern,
        },
        "output_dir": str(out_dir),
        "method": {
            "comparison_samples": "Pillow-decoded 8-bit RGBA",
            "resizing": False,
            "color_profile_conversion": False,
            "hidden_rgb_under_alpha_included": True,
            "bbox": "[left, top, right, bottom], right/bottom exclusive; null when identical",
            "mae_mse_psnr_peak": 255,
            "psnr_infinity_encoding": "psnr_db=null and psnr_is_infinite=true",
            "ssim": (
                "global SSIM per channel with K1=0.01, K2=0.03, L=255; "
                "RGBA/RGB values are arithmetic channel means"
            ),
            "abs_diff_png": (
                "opaque RGB, unamplified abs RGB difference with abs alpha difference "
                "max-composited into each visible channel"
            ),
            "side_by_side_order": ["rust", "ae"],
        },
        "summary": summary_from_totals(totals),
        "frames": frame_reports,
    }
    metrics_path = out_dir / "metrics.json"
    write_json_atomic(report, metrics_path)
    return metrics_path


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        metrics_path = run_comparison(
            rust_dir=args.rust_dir,
            ae_dir=args.ae_dir,
            frame_specs=args.frames,
            out_dir=args.out_dir,
            rust_pattern=args.rust_pattern,
            ae_pattern=args.ae_pattern,
        )
    except (ComparisonError, OSError) as exc:
        parser.error(str(exc))
    print(f"wrote frame-locked comparison: {metrics_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
