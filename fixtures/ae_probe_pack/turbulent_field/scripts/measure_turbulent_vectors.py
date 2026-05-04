#!/usr/bin/env python3
"""Measure AE Turbulent Displace vectors from coordinate-field probe renders."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from statistics import mean

from PIL import Image


SIZE = 512
GRID = 32


def is_coordinate_field_case(case_id: str) -> bool:
    if "CHECKER" in case_id or "IMPULSE" in case_id or "PINNING_EDGE" in case_id:
        return False
    return case_id.startswith("TD_")


def sample_points(width: int, height: int) -> list[tuple[int, int]]:
    points: set[tuple[int, int]] = set()

    for y in range(0, height, GRID):
        for x in range(0, width, GRID):
            points.add((x, y))
    for x in [0, width // 2, width - 1]:
        for y in [0, height // 2, height - 1]:
            points.add((x, y))

    cx = width // 2
    cy = height // 2
    for y in range(cy - 8, cy + 9):
        for x in range(cx - 8, cx + 9):
            if 0 <= x < width and 0 <= y < height:
                points.add((x, y))

    border_step = 4
    for y in list(range(0, 16, border_step)) + list(range(height - 16, height, border_step)):
        for x in range(0, width, border_step):
            points.add((x, y))
    for x in list(range(0, 16, border_step)) + list(range(width - 16, width, border_step)):
        for y in range(0, height, border_step):
            points.add((x, y))

    return sorted(points, key=lambda point: (point[1], point[0]))


def expected_parity(x: int, y: int) -> bool:
    return (((x // GRID) + (y // GRID)) & 1) == 1


def decode_source(rgba: tuple[int, int, int, int], ox: int, oy: int) -> dict[str, object] | None:
    r, g, b, a = rgba
    if a == 0:
        return None

    observed_parity = b >= 128
    candidates: list[tuple[float, int, int, bool]] = []
    for kx in range(-1, 3):
        sx = r + 256 * kx
        if sx < 0 or sx >= SIZE:
            continue
        for ky in range(-1, 3):
            sy = g + 256 * ky
            if sy < 0 or sy >= SIZE:
                continue
            parity_match = expected_parity(sx, sy) == observed_parity
            penalty = 0.0 if parity_match else SIZE * SIZE
            distance = (sx - ox) * (sx - ox) + (sy - oy) * (sy - oy) + penalty
            candidates.append((distance, sx, sy, parity_match))

    if not candidates:
        return None

    _, sx, sy, parity_match = min(candidates, key=lambda item: item[0])
    dx = sx - ox
    dy = sy - oy
    return {
        "source_x": sx,
        "source_y": sy,
        "dx": dx,
        "dy": dy,
        "magnitude": math.hypot(dx, dy),
        "parity_match": parity_match,
    }


def percentile(values: list[float], p: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    idx = min(len(ordered) - 1, max(0, round((len(ordered) - 1) * p)))
    return ordered[idx]


def summarize_samples(samples: list[dict[str, object]]) -> dict[str, object]:
    vectors = [sample for sample in samples if sample.get("decoded")]
    dx = [float(sample["dx"]) for sample in vectors]
    dy = [float(sample["dy"]) for sample in vectors]
    mag = [float(sample["magnitude"]) for sample in vectors]
    parity_mismatches = sum(1 for sample in vectors if not sample.get("parity_match"))

    if not vectors:
        return {
            "sample_count": len(samples),
            "decoded_count": 0,
            "transparent_count": len(samples),
        }

    return {
        "sample_count": len(samples),
        "decoded_count": len(vectors),
        "transparent_count": len(samples) - len(vectors),
        "parity_mismatch_count": parity_mismatches,
        "mean_dx": mean(dx),
        "mean_dy": mean(dy),
        "mean_magnitude": mean(mag),
        "p95_magnitude": percentile(mag, 0.95),
        "max_magnitude": max(mag),
        "center": next(
            (
                sample
                for sample in samples
                if sample["output_x"] == SIZE // 2 and sample["output_y"] == SIZE // 2
            ),
            None,
        ),
    }


def measure_frame(path: Path, include_samples: bool) -> tuple[dict[str, object], list[dict[str, object]]]:
    image = Image.open(path).convert("RGBA")
    width, height = image.size
    pixels = image.load()
    samples: list[dict[str, object]] = []

    for ox, oy in sample_points(width, height):
        rgba = tuple(int(channel) for channel in pixels[ox, oy])
        decoded = decode_source(rgba, ox, oy)
        sample: dict[str, object] = {
            "output_x": ox,
            "output_y": oy,
            "rgba": list(rgba),
            "decoded": decoded is not None,
        }
        if decoded is not None:
            sample.update(decoded)
        samples.append(sample)

    frame = int(path.stem.rsplit("_", 1)[-1])
    report: dict[str, object] = {
        "frame": frame,
        "path": str(path),
        "summary": summarize_samples(samples),
    }
    if include_samples:
        report["samples"] = samples
    return report, samples


def measure_pack(pack: Path, output: Path, include_samples: bool) -> dict[str, object]:
    png_root = pack / "ae_goldens" / "png8"
    if not png_root.exists():
        raise SystemExit(f"Missing AE PNG root: {png_root}")

    cases = []
    for case_dir in sorted(path for path in png_root.iterdir() if path.is_dir()):
        case_id = case_dir.name
        if not is_coordinate_field_case(case_id):
            continue
        frame_paths = sorted(case_dir.glob(f"{case_id}_*.png"))
        if not frame_paths:
            continue
        frames = []
        case_samples = []
        for path in frame_paths:
            frame, samples = measure_frame(path, include_samples)
            frames.append(frame)
            case_samples.extend(samples)
        case_vectors = [sample for sample in case_samples if sample.get("decoded")]
        cases.append(
            {
                "case_id": case_id,
                "frame_count": len(frames),
                "summary": summarize_samples(case_vectors),
                "frames": frames,
            }
        )

    report = {
        "schema": "ae-native-renderer.turbulent-vector-measurements.v1",
        "pack": str(pack),
        "png_root": str(png_root),
        "case_count": len(cases),
        "sample_grid": {
            "image_size": [SIZE, SIZE],
            "grid_step": GRID,
            "point_count": len(sample_points(SIZE, SIZE)),
        },
        "cases": cases,
    }

    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n")
    return report


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--pack",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="Turbulent probe pack root containing ae_goldens/png8.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Output JSON path. Defaults to <pack>/ae_goldens/metadata/turbulent_vector_measurements.json.",
    )
    parser.add_argument(
        "--include-samples",
        action="store_true",
        help="Include every sampled pixel in the report. Default keeps only frame and case summaries.",
    )
    args = parser.parse_args()

    output = args.output or (
        args.pack / "ae_goldens" / "metadata" / "turbulent_vector_measurements.json"
    )
    report = measure_pack(args.pack, output, args.include_samples)
    print(f"wrote {output}")
    print(f"cases={report['case_count']} sample_points={report['sample_grid']['point_count']}")


if __name__ == "__main__":
    main()
