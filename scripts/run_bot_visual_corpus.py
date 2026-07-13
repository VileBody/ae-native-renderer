#!/usr/bin/env python3
"""Render the fixed bot payload corpus and assemble optional AE comparisons."""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CORPUS = ROOT / "fixtures" / "bot_corpus"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--render-cli", type=Path, default=ROOT / "target" / "release" / "render-cli")
    parser.add_argument("--case", action="append", dest="cases")
    parser.add_argument("--reference-root", type=Path, help="Directory containing downloaded AE reference MP4s.")
    parser.add_argument("--require-reference", action="store_true", help="Fail when a selected case has no AE reference MP4.")
    parser.add_argument("--skip-video", action="store_true", help="Adapt only; do not render native MP4s.")
    return parser.parse_args()


def run(command: list[str]) -> None:
    print("+", " ".join(command))
    subprocess.run(command, cwd=ROOT, check=True)


def resolve_cases(manifest: dict[str, Any], requested: list[str] | None) -> list[dict[str, str]]:
    cases = manifest.get("cases") or []
    selected = set(requested or [])
    if selected:
        known = {str(case["id"]) for case in cases}
        missing = sorted(selected - known)
        if missing:
            raise SystemExit(f"unknown corpus case(s): {', '.join(missing)}")
        cases = [case for case in cases if str(case["id"]) in selected]
    return cases


def side_by_side(native: Path, reference: Path, out: Path) -> None:
    run([
        "ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y",
        "-i", str(native), "-i", str(reference),
        "-filter_complex", "[0:v][1:v]hstack=inputs=2[v]",
        "-map", "[v]", "-an", "-c:v", "libx264", "-crf", "18", "-preset", "veryfast", str(out),
    ])


def contact_sheet(video: Path, out: Path) -> None:
    run([
        "ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i", str(video),
        "-vf", "fps=2,scale=320:-2,tile=3x3:padding=6:margin=6", "-frames:v", "1", str(out),
    ])


def control_frames(video: Path, seconds: list[float], out: Path) -> list[str]:
    out.mkdir(parents=True, exist_ok=True)
    frames: list[str] = []
    for index, second in enumerate(seconds):
        frame = out / f"frame_{index:02d}_{second:.3f}s.png"
        run([
            "ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y",
            "-i", str(video), "-ss", f"{second:.6f}", "-frames:v", "1", str(frame),
        ])
        frames.append(str(frame))
    return frames


def reference_path(corpus: Path, case: dict[str, Any], reference_root: Path | None) -> Path | None:
    reference = case.get("reference")
    if not reference:
        return None
    root = reference_root if reference_root is not None else corpus
    return (root / str(reference)).resolve()


def main() -> None:
    args = parse_args()
    render_cli = args.render_cli.resolve()
    if not render_cli.is_file():
        raise SystemExit(f"render-cli is missing: {render_cli}; build it with cargo build -p render-cli --release")
    manifest_path = args.corpus / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    cases = resolve_cases(manifest, args.cases)
    args.out.mkdir(parents=True, exist_ok=True)
    index: list[dict[str, Any]] = []

    for case in cases:
        case_id = str(case["id"])
        case_dir = args.out / case_id
        case_dir.mkdir(parents=True, exist_ok=True)
        source = (args.corpus / str(case["input"])).resolve()
        request = case_dir / "request.json"
        run([str(render_cli), "adapt-bot-payload", "--input", str(source), "--out", str(request)])
        review: dict[str, Any] = {"case": case_id, "input": str(source), "request": str(request)}

        if not args.skip_video:
            payload = json.loads(request.read_text(encoding="utf-8"))
            native_dir = (case_dir / "native").resolve()
            native_mp4 = (case_dir / "native.mp4").resolve()
            payload["action"] = "render"
            payload["outputSpec"] = {"directory": str(native_dir), "video": str(native_mp4)}
            request.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            response = case_dir / "response.json"
            run([str(render_cli), "json", "--request", str(request), "--response", str(response)])
            review.update({"native": str(native_mp4), "response": str(response)})

            reference = reference_path(args.corpus, case, args.reference_root)
            if reference is not None and reference.is_file():
                comparison = case_dir / "side-by-side.mp4"
                side_by_side(native_mp4, reference, comparison)
                sheet = case_dir / "contact-sheet.png"
                contact_sheet(comparison, sheet)
                control_seconds = [float(value) for value in case.get("control_seconds", [0.0, 0.25, 0.5, 0.75])]
                controls = control_frames(comparison, control_seconds, case_dir / "controls")
                review.update({
                    "reference": str(reference),
                    "side_by_side": str(comparison),
                    "contact_sheet": str(sheet),
                    "control_frames": controls,
                })
            else:
                review["reference_missing"] = str(reference) if reference is not None else None
                if args.require_reference:
                    raise SystemExit(f"missing AE reference for {case_id}: {review['reference_missing']}")
        if case.get("production"):
            review["production"] = case["production"]
        if case.get("visual_requirements"):
            review["visual_requirements"] = case["visual_requirements"]
        (case_dir / "review.json").write_text(json.dumps(review, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        index.append(review)

    (args.out / "index.json").write_text(json.dumps(index, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"bot-corpus.index={args.out / 'index.json'}")


if __name__ == "__main__":
    main()
