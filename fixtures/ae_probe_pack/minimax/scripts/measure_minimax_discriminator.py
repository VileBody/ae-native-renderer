#!/usr/bin/env python3
"""Measure AE Minimax discriminator probe outputs."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from PIL import Image


CENTER = (64, 64)
ROW_Y = 64
COL_X = 64


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "input",
        help="Directory containing MMD_* case folders, or an extracted AE output root.",
    )
    parser.add_argument("--output", default="", help="Optional JSON output path.")
    args = parser.parse_args()

    root = Path(args.input).expanduser().resolve()
    case_root = find_case_root(root)
    cases = [measure_case(path) for path in sorted(case_root.iterdir()) if path.is_dir()]
    by_id = {case["case_id"]: case for case in cases}
    add_radius_expansions(cases, by_id.get("MMD_SRC_PATCH"))

    result = {
        "schema": "ae-native-renderer.minimax-discriminator-measurements.v1",
        "input": str(root),
        "case_root": str(case_root),
        "center": list(CENTER),
        "cases": cases,
        "summary": summarize(cases),
    }
    text = json.dumps(result, indent=2, ensure_ascii=False)
    if args.output:
        output = Path(args.output).expanduser().resolve()
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(text + "\n", encoding="utf-8")
    print(text)
    return 0


def find_case_root(root: Path) -> Path:
    if not root.is_dir():
        raise RuntimeError(f"input dir not found: {root}")
    direct = [path for path in root.iterdir() if path.is_dir() and path.name.startswith("MMD_")]
    if direct:
        return root
    candidates = [
        path
        for path in root.rglob("MMD_*")
        if path.is_dir() and any(child.is_file() for child in path.iterdir())
    ]
    if not candidates:
        raise RuntimeError(f"could not find MMD_* case folders under {root}")
    return candidates[0].parent


def measure_case(case_dir: Path) -> dict[str, Any]:
    frame = first_frame(case_dir)
    with Image.open(frame) as image:
        rgba = image.convert("RGBA")
        return {
            "case_id": case_dir.name,
            "frame": str(frame),
            "size": list(rgba.size),
            "rgb_nonzero_bbox": bbox_if(rgba, lambda p: max(p[:3]) > 0),
            "rgb_nonblack_bbox": bbox_if(rgba, lambda p: p[:3] != (0, 0, 0)),
            "alpha_nonzero_bbox": bbox_if(rgba, lambda p: p[3] > 0),
            "alpha_full_bbox": bbox_if(rgba, lambda p: p[3] == 255),
            "rgb_nonwhite_count": count_if(rgba, lambda p: p[:3] != (255, 255, 255)),
            "alpha_not_full_count": count_if(rgba, lambda p: p[3] != 255),
            "samples": sample_points(rgba),
            "row_y64_window": sample_row(rgba, ROW_Y, 48, 80),
            "col_x64_window": sample_col(rgba, COL_X, 48, 80),
        }


def first_frame(case_dir: Path) -> Path:
    for pattern in ("*_00000.png", "*_00000.tif", "*_00000.tiff", "*.png", "*.tif", "*.tiff"):
        files = sorted(case_dir.glob(pattern))
        if files:
            return files[0]
    raise FileNotFoundError(f"no image frames found in {case_dir}")


def bbox_if(image: Image.Image, predicate) -> list[int] | None:
    pixels = image.load()
    width, height = image.size
    min_x = width
    min_y = height
    max_x = -1
    max_y = -1
    for y in range(height):
        for x in range(width):
            if predicate(pixels[x, y]):
                min_x = min(min_x, x)
                min_y = min(min_y, y)
                max_x = max(max_x, x)
                max_y = max(max_y, y)
    if max_x < 0:
        return None
    return [min_x, min_y, max_x, max_y]


def count_if(image: Image.Image, predicate) -> int:
    pixels = image.load()
    width, height = image.size
    total = 0
    for y in range(height):
        for x in range(width):
            if predicate(pixels[x, y]):
                total += 1
    return total


def sample_points(image: Image.Image) -> dict[str, list[int]]:
    points = {
        "center": CENTER,
        "right_4": (68, 64),
        "right_8": (72, 64),
        "up_4": (64, 60),
        "down_4": (64, 68),
        "left_edge": (0, 64),
        "edge_inset_3": (3, 64),
        "edge_inset_4": (4, 64),
        "corner": (0, 0),
    }
    return {name: list(image.getpixel(xy)) for name, xy in points.items()}


def sample_row(image: Image.Image, y: int, x_start: int, x_end: int) -> list[dict[str, Any]]:
    width, height = image.size
    if y < 0 or y >= height:
        raise ValueError(f"row {y} outside image height {height}")
    xs = range(max(0, x_start), min(width - 1, x_end) + 1)
    return [{"x": x, "rgba": list(image.getpixel((x, y)))} for x in xs]


def sample_col(image: Image.Image, x: int, y_start: int, y_end: int) -> list[dict[str, Any]]:
    width, height = image.size
    if x < 0 or x >= width:
        raise ValueError(f"col {x} outside image width {width}")
    ys = range(max(0, y_start), min(height - 1, y_end) + 1)
    return [{"y": y, "rgba": list(image.getpixel((x, y)))} for y in ys]


def add_radius_expansions(cases: list[dict[str, Any]], source: dict[str, Any] | None) -> None:
    if not source:
        return
    source_bbox = source.get("rgb_nonzero_bbox") or source.get("alpha_nonzero_bbox")
    if not source_bbox:
        return
    for case in cases:
        if not case["case_id"].startswith("MMD_RAD_"):
            continue
        bbox = case.get("rgb_nonzero_bbox") or case.get("alpha_nonzero_bbox")
        if not bbox:
            continue
        case["radius_expansion_vs_source"] = {
            "left": source_bbox[0] - bbox[0],
            "top": source_bbox[1] - bbox[1],
            "right": bbox[2] - source_bbox[2],
            "bottom": bbox[3] - source_bbox[3],
        }


def summarize(cases: list[dict[str, Any]]) -> dict[str, Any]:
    radius = {}
    direction = {}
    edge = {}
    channel = {}
    for case in cases:
        case_id = case["case_id"]
        if case_id.startswith("MMD_RAD_"):
            radius[case_id] = case.get("radius_expansion_vs_source")
        elif case_id.startswith("MMD_DIR_"):
            direction[case_id] = {
                "alpha_nonzero_bbox": case.get("alpha_nonzero_bbox"),
                "rgb_nonzero_bbox": case.get("rgb_nonzero_bbox"),
            }
        elif case_id.startswith("MMD_EDGE_"):
            edge[case_id] = {
                "rgb_nonwhite_count": case.get("rgb_nonwhite_count"),
                "alpha_not_full_count": case.get("alpha_not_full_count"),
                "rgb_nonblack_bbox": case.get("rgb_nonblack_bbox"),
                "alpha_full_bbox": case.get("alpha_full_bbox"),
                "corner": case.get("samples", {}).get("corner"),
                "left_edge": case.get("samples", {}).get("left_edge"),
                "edge_inset_4": case.get("samples", {}).get("edge_inset_4"),
            }
        elif case_id.startswith("MMD_CH_"):
            channel[case_id] = {
                "center": case.get("samples", {}).get("center"),
                "right_4": case.get("samples", {}).get("right_4"),
                "right_8": case.get("samples", {}).get("right_8"),
                "alpha_nonzero_bbox": case.get("alpha_nonzero_bbox"),
                "rgb_nonzero_bbox": case.get("rgb_nonzero_bbox"),
            }
    return {
        "radius_expansions": radius,
        "direction_bboxes": direction,
        "edge_policy": edge,
        "channel_samples": channel,
    }


if __name__ == "__main__":
    raise SystemExit(main())
