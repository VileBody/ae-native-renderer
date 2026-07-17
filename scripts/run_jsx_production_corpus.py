#!/usr/bin/env python3
"""Frame-lock Rust against production AE output from archived JSX jobs.

The source archives are intentionally external to the repository.  They contain
the JSX, the exact footage files, and the AE output MP4, so every comparison is
made with one composition clock rather than a re-based subtitle clip.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import tarfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "fixtures" / "bot_corpus" / "jsx_production_manifest.json"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive-root", type=Path, required=True,
                        help="Root containing <job_id>/<job_id>_job_folder.tar.gz and output.mp4.")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--cache-root", type=Path,
                        help="Optional reusable unpacked archive cache; defaults to <out>/archives.")
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--render-cli", type=Path, default=ROOT / "target" / "release" / "render-cli")
    parser.add_argument("--case", action="append", dest="cases")
    parser.add_argument("--render-video", action="store_true",
                        help="Render a full native MP4 in addition to frame-locked controls; enables audio mux validation.")
    parser.add_argument("--discard-native-video", action="store_true",
                        help="After extracting controls, delete the full native MP4 and render frames to keep large corpus runs bounded.")
    return parser.parse_args()


def run(command: list[str], *, quiet: bool = False) -> None:
    print("+", " ".join(str(part) for part in command))
    subprocess.run(
        [str(part) for part in command],
        cwd=ROOT,
        check=True,
        stdout=subprocess.DEVNULL if quiet else None,
    )


def select_cases(manifest: dict[str, Any], requested: list[str] | None) -> list[dict[str, Any]]:
    cases = manifest["cases"]
    if not requested:
        return cases
    wanted = set(requested)
    known = {str(case["id"]) for case in cases}
    missing = sorted(wanted - known)
    if missing:
        raise SystemExit(f"unknown case(s): {', '.join(missing)}")
    return [case for case in cases if case["id"] in wanted]


def unpack_job(archive_root: Path, job_id: str, cache_root: Path) -> Path:
    extracted_matches = sorted(archive_root.glob(f"**/job_folder/{job_id}/app/render.jsx"))
    if extracted_matches:
        return extracted_matches[0].parent
    extracted_matches = sorted(archive_root.glob(f"**/{job_id}/app/render.jsx"))
    if extracted_matches:
        return extracted_matches[0].parent
    app_root = cache_root / job_id / "app"
    if (app_root / "render.jsx").is_file():
        return app_root
    archive = archive_root / job_id / f"{job_id}_job_folder.tar.gz"
    if not archive.is_file():
        matches = sorted(archive_root.glob(f"**/{job_id}_job_folder.tar.gz"))
        if matches:
            archive = matches[0]
    if not archive.is_file():
        raise SystemExit(f"missing archive: {archive}")
    cache_root.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive, "r:gz") as bundle:
        prefix = f"{job_id}/"
        for member in bundle.getmembers():
            if not member.name.startswith(prefix) or member.name.startswith("/") or ".." in Path(member.name).parts:
                continue
            bundle.extract(member, cache_root, filter="data")
    if not (app_root / "render.jsx").is_file():
        raise SystemExit(f"archive has no app/render.jsx: {archive}")
    return app_root


def text_layer_window(request: dict[str, Any], text: str) -> tuple[float, float]:
    matches = [layer for layer in request.get("text_layers", []) if str(layer.get("name", "")).upper() == text]
    if not matches:
        raise SystemExit(f"text layer not found in JSX request: {text}")
    starts = [float(layer["in_point"]) for layer in matches]
    ends = [float(layer["out_point"]) for layer in matches]
    return min(starts), max(ends)


def state_times(start: float, end: float, states: list[str]) -> list[float]:
    duration = end - start
    fractions = {
        "entrance": 0.08, "outline": 0.06, "first_group": 0.18,
        "fill_start": 0.28, "second_group": 0.42, "accumulation": 0.52,
        "peak": 0.68, "exit": 0.92,
    }
    return [start + duration * fractions[state] for state in states]


def control_times_for_case(case: dict[str, Any], start: float, end: float, fps: float) -> tuple[list[str], list[float], list[int]]:
    if case.get("control_frames") is not None:
        frames = [int(frame) for frame in case["control_frames"]]
        return ([f"frame_{frame}" for frame in frames], [frame / fps for frame in frames], frames)
    if case.get("control_times") is not None:
        times = [float(time) for time in case["control_times"]]
        return ([f"{time:.3f}s" for time in times], times, [round(time * fps) for time in times])
    states = list(case["states"])
    times = state_times(start, end, states)
    return states, times, [round(time * fps) for time in times]


def reference_for_case(archive_root: Path, case: dict[str, Any]) -> Path:
    if case.get("reference"):
        reference = Path(str(case["reference"]))
        return reference if reference.is_absolute() else (ROOT / reference)
    job_id = str(case["job_id"])
    reference = archive_root / job_id / "output.mp4"
    if reference.is_file():
        return reference
    archive_matches = sorted(archive_root.glob(f"**/{job_id}_job_folder.tar.gz"))
    for archive in archive_matches:
        sibling = archive.parent / "output.mp4"
        if sibling.is_file():
            return sibling
    matches = sorted(archive_root.glob(f"**/{job_id}/output.mp4"))
    if matches:
        return matches[0]
    matches = sorted(archive_root.glob(f"**/{job_id}_output.mp4"))
    if matches:
        return matches[0]
    case_id = str(case.get("id") or "")
    if case_id:
        matches = sorted(archive_root.glob(f"**/{case_id}/output.mp4"))
        if matches:
            return matches[0]
    return reference


def ae_frame(reference: Path, frame: int, out: Path) -> None:
    run(["ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i", reference,
         "-vf", f"select=eq(n\\,{frame})", "-frames:v", "1", out])


def video_frame(reference: Path, frame: int, out: Path) -> None:
    ae_frame(reference, frame, out)


def hstack(native: Path, ae: Path, out: Path) -> None:
    run(["ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i", native, "-i", ae,
         "-filter_complex", "[0:v][1:v]hstack=inputs=2", "-frames:v", "1", out])


def controls_movie(frames: list[Path], out: Path) -> None:
    sequence = out.parent / "frames.txt"
    sequence.write_text(
        "".join(f"file '{frame.resolve()}'\nduration 1\n" for frame in frames)
        + f"file '{frames[-1].resolve()}'\n",
        encoding="utf-8",
    )
    run(["ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0",
         "-i", sequence, "-vf", "fps=2,format=yuv420p", "-c:v", "libx264", "-crf", "18", out])


def contact_sheet(frames: list[Path], out: Path) -> None:
    sequence = out.parent / "contact-frames.txt"
    sequence.write_text(
        "".join(f"file '{frame.resolve()}'\nduration 1\n" for frame in frames)
        + f"file '{frames[-1].resolve()}'\n",
        encoding="utf-8",
    )
    run(["ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0",
         "-i", sequence, "-vf", "fps=1,scale=320:-2,tile=2x5:padding=6:margin=6", "-frames:v", "1", out])


def capability_summary(response: Path) -> dict[str, Any]:
    capabilities = json.loads(response.read_text(encoding="utf-8")).get("capabilities", {})
    return {
        "complete": bool(capabilities.get("complete")),
        "supported": len(capabilities.get("supported", [])),
        "approximate": len(capabilities.get("approximate", [])),
        "not_implemented": [item.get("code") for item in capabilities.get("not_implemented", [])],
        "unsupported": [item.get("code") for item in capabilities.get("unsupported", [])],
    }


def infer_f3_ids_from_case_metadata(case: dict[str, Any], app_root: Path) -> list[str]:
    """Recover public F3 ids when legacy JSX comments omit stable ids.

    Older /bigtest exports sometimes contain the F3 injected section and timing
    but no machine-readable effect id.  The acceptance manifest and exported job
    folder still preserve the public selector label, so use that metadata to
    keep the corpus request equivalent to the user's actual bot choice.
    """
    haystack = " ".join(
        [str(case.get("id") or ""), str(app_root)]
        + [str(item) for item in case.get("requirements", [])]
    ).lower()
    tokens = [
        ("минимакс", "minimax"),
        ("minimax", "minimax"),
        ("неон", "neon_extract"),
        ("neon", "neon_extract"),
        ("старая камера", "old_camera"),
        ("old camera", "old_camera"),
        ("old_camera", "old_camera"),
    ]
    detected = [effect_id for needle, effect_id in tokens if needle in haystack]
    return sorted(set(detected))


def patch_empty_f3_ids_from_case_metadata(request: dict[str, Any], case: dict[str, Any], app_root: Path) -> None:
    inferred = infer_f3_ids_from_case_metadata(case, app_root)
    if not inferred:
        return
    for operation in request.get("visualOps", []):
        if operation.get("type") != "hook.f3.effect.v1":
            continue
        params = operation.setdefault("params", {})
        ids = params.get("detected_effect_ids")
        if isinstance(ids, list) and not ids:
            params["detected_effect_ids"] = inferred
            params["detected_effect_ids_source"] = "jsx-production-corpus-metadata"


def main() -> None:
    args = parse_args()
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    default_fps = float(manifest.get("fps", 24))
    render_cli = args.render_cli.resolve()
    if not render_cli.is_file():
        raise SystemExit(f"render-cli is missing: {render_cli}")
    args.out.mkdir(parents=True, exist_ok=True)
    cache_root = args.cache_root.resolve() if args.cache_root else args.out / "archives"
    index: list[dict[str, Any]] = []
    first_comparisons: list[Path] = []

    for case in select_cases(manifest, args.cases):
        case_dir = args.out / str(case["id"])
        case_dir.mkdir(parents=True, exist_ok=True)
        app_root = unpack_job(args.archive_root, str(case["job_id"]), cache_root).resolve()
        extracted = case_dir / "request.extracted.json"
        run([render_cli, "extract-jsx-request", "--jsx", app_root / "render.jsx", "--out", extracted])
        request = json.loads(extracted.read_text(encoding="utf-8"))
        patch_empty_f3_ids_from_case_metadata(request, case, app_root)
        comps = request.get("compsSpec") or []
        comp_fps = comps[0].get("fps") if comps and isinstance(comps[0], dict) else None
        fps = float(comp_fps or default_fps)
        explicit_window = case.get("window")
        if explicit_window is not None:
            if (
                not isinstance(explicit_window, list)
                or len(explicit_window) != 2
                or not all(isinstance(value, (int, float)) for value in explicit_window)
            ):
                raise SystemExit(f"invalid explicit window for case {case['id']}")
            start, end = (float(explicit_window[0]), float(explicit_window[1]))
            if end <= start:
                raise SystemExit(f"invalid explicit window for case {case['id']}")
        else:
            start, end = text_layer_window(request, str(case["text"]))
        labels, times, frames = control_times_for_case(case, start, end, fps)
        request["policy"] = {"onUnsupported": "report"}
        request["assetsSpec"] = {"root": str(app_root)}
        native_dir = (case_dir / "native").resolve()
        native_video = case_dir / "native.mp4"
        request["outputSpec"] = {"directory": str(native_dir)}
        if args.render_video:
            request["outputSpec"]["video"] = str(native_video.resolve())
        else:
            request["outputSpec"]["frames"] = frames
        request["action"] = "render"
        render_request = case_dir / "request.render.json"
        render_request.write_text(json.dumps(request, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        response = case_dir / "response.json"
        run([render_cli, "json", "--request", render_request, "--response", response], quiet=True)

        reference = reference_for_case(args.archive_root, case)
        if not reference.is_file():
            raise SystemExit(f"missing AE reference: {reference}")
        comparisons: list[Path] = []
        controls: list[dict[str, Any]] = []
        for label, time, frame in zip(labels, times, frames, strict=True):
            native = native_dir / "render" / "frames" / f"frame_{frame:06d}.png"
            if args.render_video:
                native = case_dir / f"native_{frame:06d}.png"
                video_frame(native_video, frame, native)
            if not native.is_file():
                raise SystemExit(f"native frame missing: {native}")
            ae = case_dir / f"ae_{frame:06d}.png"
            comparison = case_dir / f"side_by_side_{frame:06d}.png"
            ae_frame(reference, frame, ae)
            hstack(native, ae, comparison)
            comparisons.append(comparison)
            controls.append({"state": label, "time": time, "frame": frame, "native": str(native), "ae": str(ae), "side_by_side": str(comparison)})
        movie = case_dir / "side-by-side.mp4"
        controls_movie(comparisons, movie)
        first_comparisons.append(comparisons[0])
        review = {
            "case": case["id"], "type": case["type"], "job_id": case["job_id"], "text": case["text"],
            "window": [start, end], "fps": fps, "requirements": case["requirements"], "controls": controls,
            "side_by_side_video": str(movie), "response": str(response),
            "capabilities": capability_summary(response),
        }
        if args.render_video:
            review["native_video"] = str(native_video)
            if args.discard_native_video and native_video.exists():
                native_video.unlink()
                review["native_video_discarded"] = True
            if args.discard_native_video and native_dir.exists():
                shutil.rmtree(native_dir)
                review["native_render_dir_discarded"] = True
        (case_dir / "review.json").write_text(json.dumps(review, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        index.append(review)

    contact = args.out / "contact-sheet.png"
    contact_sheet(first_comparisons, contact)
    (args.out / "index.json").write_text(json.dumps({"cases": index, "contact_sheet": str(contact)}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"jsx-production-corpus.index={args.out / 'index.json'}")


if __name__ == "__main__":
    main()
