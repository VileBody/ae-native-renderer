#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path

from PIL import Image


def channel_bbox(path: Path, mode: str) -> dict | None:
    with Image.open(path) as im:
        rgba = im.convert("RGBA")
        w, h = rgba.size
        xs = []
        ys = []
        pix = rgba.load()
        for y in range(h):
            for x in range(w):
                r, g, b, a = pix[x, y]
                if mode == "alpha":
                    keep = a != 0
                else:
                    keep = (r | g | b) != 0
                if keep:
                    xs.append(x)
                    ys.append(y)
        if not xs:
            return None
        return {
            "nonzero": len(xs),
            "bbox": {
                "left": min(xs),
                "top": min(ys),
                "right_inclusive": max(xs),
                "bottom_inclusive": max(ys),
                "right_exclusive": max(xs) + 1,
                "bottom_exclusive": max(ys) + 1,
            },
        }


def image_measure(path: Path) -> dict:
    with Image.open(path) as im:
        w, h = im.size
    return {
        "path": str(path),
        "width": w,
        "height": h,
        "alpha": channel_bbox(path, "alpha") or {"nonzero": 0, "bbox": None},
        "rgb_nonblack": channel_bbox(path, "rgb") or {"nonzero": 0, "bbox": None},
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("root", help="Extracted AE output root or png directory")
    ap.add_argument("--out", default="", help="Optional JSON output path")
    args = ap.parse_args()

    root = Path(args.root).expanduser().resolve()
    images = sorted(
        p for p in root.rglob("*")
        if p.is_file() and p.suffix.lower() in {".png", ".tif", ".tiff"}
    )
    rows = [image_measure(path) for path in images]
    result = {"root": str(root), "images": len(images), "rows": rows}
    text = json.dumps(result, indent=2, sort_keys=True)
    if args.out:
        out = Path(args.out).expanduser().resolve()
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text + "\n", encoding="utf-8")
    print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
