#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path

from PIL import Image


def measure_png(path: Path) -> dict[str, object]:
    with Image.open(path) as im:
        rgba = im.convert("RGBA")
        pixels = list(rgba.getdata())
    nonzero = [p for p in pixels if p[3] > 0]
    non_bg = [p for p in pixels if p != (0, 0, 0, 255) and p != (0, 0, 0, 0)]
    return {
        "path": str(path),
        "size": list(rgba.size),
        "non_bg_count": len(non_bg),
        "alpha_nonzero_count": len(nonzero),
        "max_rgba": [
            max((p[i] for p in pixels), default=0)
            for i in range(4)
        ],
        "brightest_alpha_then_rgb": list(max(pixels, key=lambda p: (p[3], p[0], p[1], p[2]))),
    }


def first_png(case_dir: Path) -> Path | None:
    files = sorted(case_dir.glob("*.png"))
    return files[0] if files else None


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", required=True, help="Extracted AE output root")
    ap.add_argument("--out", default="")
    args = ap.parse_args()

    root = Path(args.root)
    results: dict[str, object] = {}
    for case_dir in sorted(root.rglob("TRFLIVE_*")):
        if not case_dir.is_dir():
            continue
        png = first_png(case_dir)
        if png is not None:
            results[case_dir.name] = measure_png(png)

    method_logs = sorted(root.rglob("text_fill_alpha_method.txt"))
    payload = {
        "root": str(root),
        "cases": results,
        "method_logs": [str(p) for p in method_logs],
        "method_log_text": method_logs[0].read_text(encoding="utf-8") if method_logs else "",
    }
    text = json.dumps(payload, indent=2, ensure_ascii=False) + "\n"
    if args.out:
        Path(args.out).write_text(text, encoding="utf-8")
    else:
        print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
