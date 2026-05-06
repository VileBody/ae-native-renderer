#!/usr/bin/env python3
"""Export native Turbulent Displace samples through render-cli.

This is the bridge that keeps vector fitting honest: the Python scripts still
own AE decode and statistics, but native dx/dy samples come from Rust code.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parents[3]
sys.path.insert(0, str(SCRIPT_DIR))

import compare_turbulent_native_vectors as compare


def params_json(params: compare.FieldParams) -> dict[str, Any]:
    return {
        "displacement": params.displacement_type,
        "amount": params.amount,
        "size": params.size,
        "offset": list(params.offset),
        "complexity": params.complexity,
        "evolution": params.evolution,
        "random_seed": params.random_seed,
        "pinning": params.pinning,
        "resize_layer": params.resize_layer,
    }


def build_request(
    measurements_path: Path,
    measurements: dict[str, Any],
    args: argparse.Namespace,
) -> dict[str, Any]:
    request_frames = []
    selected_cases = compare.select_cases(measurements, args)
    for case in selected_cases:
        case_id = str(case["case_id"])
        for frame in case.get("frames", []):
            frame_number = int(frame["frame"])
            params, notes = compare.case_params(case_id, frame_number)
            if params is None:
                continue
            samples = compare.load_samples_for_frame(frame, measurements_path, measurements)
            points = [
                [int(sample["output_x"]), int(sample["output_y"])]
                for sample in samples
                if "output_x" in sample and "output_y" in sample
            ]
            request_frames.append(
                {
                    "case_id": case_id,
                    "frame": frame_number,
                    "time": frame_number / compare.FPS,
                    "params": params_json(params),
                    "points": points,
                    "notes": notes,
                }
            )

    return {
        "schema": "ae-native-renderer.turbulent-native-samples-request.v1",
        "source_measurements": str(measurements_path),
        "width": compare.WIDTH,
        "height": compare.HEIGHT,
        "frames": request_frames,
    }


def run_render_cli(request_path: Path, output_path: Path, args: argparse.Namespace) -> None:
    if args.render_cli:
        cmd = [
            str(args.render_cli),
            "turbulent-samples",
            "--request",
            str(request_path),
            "--out",
            str(output_path),
        ]
    else:
        cmd = [
            "cargo",
            "run",
            "-p",
            "render-cli",
            "--",
            "turbulent-samples",
            "--request",
            str(request_path),
            "--out",
            str(output_path),
        ]
    print("$ " + " ".join(cmd), flush=True)
    subprocess.run(cmd, cwd=REPO_ROOT, check=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--measurements", type=Path, required=True)
    parser.add_argument("--case", action="append", default=[])
    parser.add_argument("--all-cases", action="store_true")
    parser.add_argument("--request-output", type=Path, default=None)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--render-cli",
        type=Path,
        default=None,
        help="Use an existing render-cli binary instead of cargo run.",
    )
    parser.add_argument(
        "--request-only",
        action="store_true",
        help="Write the render-cli request but do not execute it.",
    )
    args = parser.parse_args()

    measurements = json.loads(args.measurements.read_text())
    request = build_request(args.measurements, measurements, args)
    request_path = args.request_output or args.output.with_suffix(".request.json")
    request_path.parent.mkdir(parents=True, exist_ok=True)
    request_path.write_text(json.dumps(request, indent=2) + "\n")
    print(
        "native-samples.request "
        f"frames={len(request['frames'])} "
        f"points={sum(len(frame['points']) for frame in request['frames'])} "
        f"path={request_path}"
    )
    if args.request_only:
        return
    run_render_cli(request_path, args.output, args)


if __name__ == "__main__":
    main()
