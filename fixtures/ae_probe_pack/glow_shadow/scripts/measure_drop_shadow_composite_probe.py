#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from PIL import Image


SAMPLE_POINTS = {
    "center": (256, 256),
    "offset_right": (288, 256),
    "background": (200, 256),
}


def first_frame(case_dir: Path) -> Path:
    for pattern in ("*_00000.png", "*_00000.tif", "*_00000.tiff", "*.png", "*.tif", "*.tiff"):
        files = sorted(case_dir.glob(pattern))
        if files:
            return files[0]
    raise FileNotFoundError(f"no image frames found in {case_dir}")


def alpha_bbox(image: Image.Image) -> dict[str, Any]:
    rgba = image.convert("RGBA")
    pix = rgba.load()
    width, height = rgba.size
    xs: list[int] = []
    ys: list[int] = []
    max_alpha = 0
    for y in range(height):
        for x in range(width):
            alpha = pix[x, y][3]
            if alpha:
                xs.append(x)
                ys.append(y)
                max_alpha = max(max_alpha, alpha)
    if not xs:
        return {"bbox": None, "nonzero_alpha": 0, "max_alpha": 0}
    return {
        "bbox": [min(xs), min(ys), max(xs), max(ys)],
        "nonzero_alpha": len(xs),
        "max_alpha": max_alpha,
    }


def measure_case(case_dir: Path) -> dict[str, Any]:
    frame = first_frame(case_dir)
    with Image.open(frame) as image:
        rgba = image.convert("RGBA")
        samples = {
            name: list(rgba.getpixel(xy))
            for name, xy in SAMPLE_POINTS.items()
        }
        return {
            "case_id": case_dir.name,
            "frame": str(frame),
            "samples": samples,
            "alpha": alpha_bbox(rgba),
        }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("input", help="Directory containing DSC_* case folders")
    parser.add_argument("--output", default="", help="Optional JSON output path")
    args = parser.parse_args()

    root = Path(args.input).expanduser().resolve()
    if not root.is_dir():
        raise RuntimeError(f"input dir not found: {root}")

    cases = [
        measure_case(path)
        for path in sorted(root.iterdir())
        if path.is_dir() and path.name.startswith("DSC_")
    ]
    result = {
        "schema": "ae-native-renderer.drop-shadow-composite-measurements.v1",
        "input": str(root),
        "sample_points": {name: list(xy) for name, xy in SAMPLE_POINTS.items()},
        "cases": cases,
    }
    text = json.dumps(result, indent=2, ensure_ascii=False)
    if args.output:
        output = Path(args.output).expanduser().resolve()
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(text + "\n", encoding="utf-8")
    print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
