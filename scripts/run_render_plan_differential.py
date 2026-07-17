#!/usr/bin/env python3
"""Render canonical RenderPlan/native requests and build frame-locked review assets."""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--request", type=Path, action="append", required=True,
                        help="Canonical ae-native-renderer.render-request.v1 JSON.")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--render-cli", type=Path, default=ROOT / "target" / "release" / "render-cli")
    parser.add_argument("--reference", type=Path, action="append",
                        help="Optional AE reference MP4. Order must match --request order.")
    parser.add_argument("--control-time", type=float, action="append", default=[],
                        help="Seconds for side-by-side control frames. Defaults to 0, 25, 50 and 75 percent duration.")
    parser.add_argument("--heatmap", action="store_true",
                        help="Also write absolute RGB difference heatmaps for control frames.")
    return parser.parse_args()


def run(command: list[str | Path]) -> None:
    print("+", " ".join(str(part) for part in command))
    subprocess.run([str(part) for part in command], cwd=ROOT, check=True)


def read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def request_duration(request: dict[str, Any]) -> float:
    main = request.get("projectSpec", {}).get("mainCompName")
    comps = request.get("compsSpec") or []
    for comp in comps:
        if comp.get("name") == main:
            return float(comp.get("dur") or 0.0)
    return float(comps[0].get("dur") if comps else 0.0)


def request_id(request_path: Path, request: dict[str, Any]) -> str:
    raw = request.get("requestId") or request.get("request_id") or request_path.stem
    return "".join(char if char.isalnum() or char in "-_" else "_" for char in str(raw))


def default_control_times(duration: float) -> list[float]:
    if duration <= 0:
        return [0.0]
    return [duration * fraction for fraction in (0.0, 0.25, 0.5, 0.75)]


def extract_frame(video: Path, time: float, out: Path) -> None:
    run([
        "ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y",
        "-ss", f"{time:.6f}", "-i", video, "-frames:v", "1", out,
    ])


def side_by_side(left: Path, right: Path, out: Path) -> None:
    run([
        "ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y",
        "-i", left, "-i", right,
        "-filter_complex", "[0:v][1:v]hstack=inputs=2[v]",
        "-map", "[v]", "-frames:v", "1", out,
    ])


def heatmap(left: Path, right: Path, out: Path) -> None:
    run([
        "ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y",
        "-i", left, "-i", right,
        "-filter_complex", "[0:v][1:v]blend=all_mode=difference,format=rgb24[v]",
        "-map", "[v]", "-frames:v", "1", out,
    ])


def contact_sheet(frames: list[Path], out: Path) -> None:
    if not frames:
        return
    sequence = out.parent / "contact-frames.txt"
    sequence.write_text(
        "".join(f"file '{frame.resolve()}'\nduration 1\n" for frame in frames)
        + f"file '{frames[-1].resolve()}'\n",
        encoding="utf-8",
    )
    run([
        "ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y",
        "-f", "concat", "-safe", "0", "-i", sequence,
        "-vf", "fps=1,scale=360:-2,tile=2x3:padding=6:margin=6",
        "-frames:v", "1", out,
    ])


def capability_summary(response_path: Path) -> dict[str, Any]:
    response = read_json(response_path)
    capabilities = response.get("capabilities") or {}
    return {
        "status": response.get("status"),
        "complete": capabilities.get("complete"),
        "supported": len(capabilities.get("supported") or []),
        "approximate": len(capabilities.get("approximate") or []),
        "not_implemented": [item.get("code") for item in capabilities.get("not_implemented") or []],
        "unsupported": [item.get("code") for item in capabilities.get("unsupported") or []],
    }


def main() -> None:
    args = parse_args()
    render_cli = args.render_cli.resolve()
    if not render_cli.is_file():
        raise SystemExit(f"render-cli is missing: {render_cli}; build it with cargo build -p render-cli --release")
    references = args.reference or []
    if references and len(references) != len(args.request):
        raise SystemExit("--reference count must match --request count")
    args.out.mkdir(parents=True, exist_ok=True)
    index: list[dict[str, Any]] = []

    for idx, request_path in enumerate(args.request):
        request_path = request_path.resolve()
        request = read_json(request_path)
        case_id = request_id(request_path, request)
        case_dir = args.out / case_id
        case_dir.mkdir(parents=True, exist_ok=True)
        duration = request_duration(request)
        control_times = args.control_time or default_control_times(duration)

        native_dir = (case_dir / "native").resolve()
        native_mp4 = (case_dir / "native.mp4").resolve()
        request["action"] = "render"
        request.setdefault("outputSpec", {})
        request["outputSpec"]["directory"] = str(native_dir)
        request["outputSpec"]["video"] = str(native_mp4)
        render_request = case_dir / "request.render.json"
        write_json(render_request, request)
        response = case_dir / "response.json"
        run([render_cli, "json", "--request", render_request, "--response", response])

        review: dict[str, Any] = {
            "case": case_id,
            "request": str(render_request),
            "native": str(native_mp4),
            "response": str(response),
            "capabilities": capability_summary(response),
            "controls": [],
        }

        if references:
            reference = references[idx].resolve()
            comparisons: list[Path] = []
            for frame_idx, time in enumerate(control_times):
                native_frame = case_dir / f"native_{frame_idx:02d}_{time:.3f}s.png"
                reference_frame = case_dir / f"ae_{frame_idx:02d}_{time:.3f}s.png"
                comparison = case_dir / f"side_by_side_{frame_idx:02d}_{time:.3f}s.png"
                extract_frame(native_mp4, time, native_frame)
                extract_frame(reference, time, reference_frame)
                side_by_side(native_frame, reference_frame, comparison)
                record = {
                    "time": time,
                    "native": str(native_frame),
                    "reference": str(reference_frame),
                    "side_by_side": str(comparison),
                }
                if args.heatmap:
                    heat = case_dir / f"heatmap_{frame_idx:02d}_{time:.3f}s.png"
                    heatmap(native_frame, reference_frame, heat)
                    record["heatmap"] = str(heat)
                comparisons.append(comparison)
                review["controls"].append(record)
            sheet = case_dir / "contact-sheet.png"
            contact_sheet(comparisons, sheet)
            review["reference"] = str(reference)
            review["contact_sheet"] = str(sheet)

        write_json(case_dir / "review.json", review)
        index.append(review)

    write_json(args.out / "index.json", {"cases": index})
    print(f"render-plan-differential.index={args.out / 'index.json'}")


if __name__ == "__main__":
    main()
