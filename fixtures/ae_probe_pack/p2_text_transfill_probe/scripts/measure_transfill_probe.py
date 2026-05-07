#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path

from PIL import Image


def find_first_image(root: Path, case_id: str) -> Path | None:
    matches = sorted(
        p
        for p in root.rglob("*")
        if p.is_file()
        and case_id in p.parts + (p.name,)
        and p.suffix.lower() in {".png", ".tif", ".tiff"}
    )
    return matches[0] if matches else None


def summarize_image(path: Path) -> dict[str, object]:
    with Image.open(path) as im:
        rgba = im.convert("RGBA")
        pixels = list(rgba.getdata())
    non_bg = [p for p in pixels if p[3] != 0 or p[:3] != (0, 0, 0)]
    alpha_pixels = [p for p in pixels if p[3] != 0]
    max_alpha = max((p[3] for p in pixels), default=0)
    max_rgb_sum = max((p[0] + p[1] + p[2] for p in pixels), default=0)
    bright = max(pixels, key=lambda p: (p[3], p[0] + p[1] + p[2]))
    return {
        "path": str(path),
        "size": list(rgba.size),
        "non_black_or_alpha_count": len(non_bg),
        "alpha_nonzero_count": len(alpha_pixels),
        "max_alpha": max_alpha,
        "max_rgb_sum": max_rgb_sum,
        "brightest_alpha_then_rgb": list(bright),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", required=True, help="Extracted AE output root")
    ap.add_argument("--case", action="append", default=[])
    args = ap.parse_args()

    root = Path(args.root).expanduser().resolve()
    case_ids = args.case or [
        "TRF_WHT_A25_TRANSPARENT",
        "TRF_WHT_A50_TRANSPARENT",
        "TRF_WHT_A128_TRANSPARENT",
        "TRF_WHT_A255_TRANSPARENT",
        "TRF_RED_A128_TRANSPARENT",
        "TRF_WHT_A25_BLACK",
        "TRF_WHT_A50_BLACK",
        "TRF_WHT_A128_BLACK",
        "TRF_WHT_A255_BLACK",
        "TRF_RED_A128_BLUE",
    ]
    rows = []
    for case_id in case_ids:
        path = find_first_image(root, case_id)
        rows.append({"case": case_id, "found": path is not None, **(summarize_image(path) if path else {})})
    print(json.dumps(rows, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
