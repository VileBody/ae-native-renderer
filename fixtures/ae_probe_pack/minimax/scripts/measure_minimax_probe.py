#!/usr/bin/env python3
"""Measure rendered AE Minimax probe PNGs.

Outputs bbox data and row samples for the Minimax enum/channel matrix.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


CASE_IDS = [
    "MINIMAX_PRE",
    "MINIMAX_OP1_CH1_R0",
    "MINIMAX_OP1_CH1_R12",
    "MINIMAX_OP1_CH2_R0",
    "MINIMAX_OP1_CH2_R12",
    "MINIMAX_OP2_CH1_R0",
    "MINIMAX_OP2_CH1_R12_EFF050",
    "MINIMAX_OP2_CH2_R0",
    "MINIMAX_OP2_CH2_R12",
]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--pack",
        type=Path,
        default=Path("fixtures/ae_probe_pack/minimax"),
        help="Path to the minimax probe pack root.",
    )
    parser.add_argument("--y", type=int, default=256)
    parser.add_argument("--x-start", type=int, default=211)
    parser.add_argument("--x-end", type=int, default=300)
    args = parser.parse_args()

    try:
        from PIL import Image
    except ImportError as exc:
        raise SystemExit("Pillow is required: python3 -m pip install Pillow") from exc

    pack = args.pack
    output = {
        "pack": str(pack),
        "row_sample_request": {
            "y": args.y,
            "x_start": args.x_start,
            "x_end": args.x_end,
        },
        "cases": [],
    }

    for case_id in CASE_IDS:
        png = first_png(pack / "ae_goldens" / "png" / case_id)
        if png is None:
            output["cases"].append({"id": case_id, "status": "missing_png"})
            continue
        with Image.open(png) as image:
            rgba = image.convert("RGBA")
            case = {
                "id": case_id,
                "status": "ok",
                "png": str(png),
                "size": list(rgba.size),
                "rgb_non_background_bbox": rgb_non_background_bbox(rgba),
                "alpha_nonzero_bbox": alpha_nonzero_bbox(rgba),
                "row_samples": row_samples(rgba, args.y, args.x_start, args.x_end),
            }
            output["cases"].append(case)

    out_dir = pack / "ae_goldens" / "metadata"
    out_dir.mkdir(parents=True, exist_ok=True)
    out_file = out_dir / "minimax_measurements.json"
    out_file.write_text(json.dumps(output, indent=2), encoding="utf-8")
    print(f"wrote {out_file}")
    return 0


def first_png(case_dir: Path) -> Path | None:
    files = sorted(case_dir.glob("*.png"))
    return files[0] if files else None


def rgb_non_background_bbox(image) -> list[int] | None:
    pixels = image.load()
    width, height = image.size
    bg = pixels[0, 0][:3]
    min_x = width
    min_y = height
    max_x = -1
    max_y = -1
    for y in range(height):
        for x in range(width):
            r, g, b, _a = pixels[x, y]
            if (r, g, b) != bg:
                min_x = min(min_x, x)
                min_y = min(min_y, y)
                max_x = max(max_x, x)
                max_y = max(max_y, y)
    if max_x < 0:
        return None
    return [min_x, min_y, max_x, max_y]


def alpha_nonzero_bbox(image) -> list[int] | None:
    pixels = image.load()
    width, height = image.size
    min_x = width
    min_y = height
    max_x = -1
    max_y = -1
    for y in range(height):
        for x in range(width):
            if pixels[x, y][3] > 0:
                min_x = min(min_x, x)
                min_y = min(min_y, y)
                max_x = max(max_x, x)
                max_y = max(max_y, y)
    if max_x < 0:
        return None
    return [min_x, min_y, max_x, max_y]


def row_samples(image, y: int, x_start: int, x_end: int) -> list[dict[str, object]]:
    pixels = image.load()
    width, height = image.size
    if y < 0 or y >= height:
        raise ValueError(f"row y={y} outside image height {height}")
    samples = []
    for x in range(max(0, x_start), min(width - 1, x_end) + 1):
        samples.append({"x": x, "rgba": list(pixels[x, y])})
    return samples


if __name__ == "__main__":
    raise SystemExit(main())
