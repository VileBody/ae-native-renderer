#!/usr/bin/env python3
"""Generate deterministic PNG primitives for AE/native conformance cases."""

from __future__ import annotations

import json
import math
import struct
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets" / "primitives"
SIZE = 256
NUMBERED_FRAMES = 90


def chunk(kind: bytes, data: bytes) -> bytes:
    return (
        struct.pack(">I", len(data))
        + kind
        + data
        + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
    )


def save_png(path: Path, width: int, height: int, pixels: list[tuple[int, int, int, int]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    rows = []
    for y in range(height):
        row = bytearray([0])
        for x in range(width):
            row.extend(pixels[y * width + x])
        rows.append(bytes(row))
    raw = b"".join(rows)
    data = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )
    path.write_bytes(data)


def clamp_u8(value: float) -> int:
    return max(0, min(255, int(round(value))))


def image(width: int, height: int, fn) -> list[tuple[int, int, int, int]]:
    return [fn(x, y) for y in range(height) for x in range(width)]


def solid(width: int, height: int, rgba: tuple[int, int, int, int]):
    return image(width, height, lambda _x, _y: rgba)


def alpha_square(width: int, height: int):
    margin = width // 4
    return image(
        width,
        height,
        lambda x, y: (255, 255, 255, 255)
        if margin <= x < width - margin and margin <= y < height - margin
        else (0, 0, 0, 0),
    )


def impulse(width: int, height: int):
    cx = width // 2
    cy = height // 2
    return image(
        width,
        height,
        lambda x, y: (255, 255, 255, 255) if x == cx and y == cy else (0, 0, 0, 0),
    )


def impulse_grid(width: int, height: int):
    points = {(width // 4, height // 4), (width // 2, height // 2), (3 * width // 4, 3 * height // 4)}
    return image(
        width,
        height,
        lambda x, y: (255, 255, 255, 255) if (x, y) in points else (0, 0, 0, 0),
    )


def alpha_ramp(width: int, height: int):
    den = max(1, width - 1)
    return image(width, height, lambda x, _y: (255, 255, 255, clamp_u8(255 * x / den)))


def luma_ramp(width: int, height: int):
    den = max(1, width - 1)
    return image(width, height, lambda x, _y: (clamp_u8(255 * x / den),) * 3 + (255,))


def coordinate_field(width: int, height: int):
    xden = max(1, width - 1)
    yden = max(1, height - 1)
    return image(
        width,
        height,
        lambda x, y: (clamp_u8(255 * x / xden), clamp_u8(255 * y / yden), 0, 255),
    )


def checkerboard(width: int, height: int, cell: int = 16):
    return image(
        width,
        height,
        lambda x, y: ((235, 235, 235, 255) if ((x // cell) + (y // cell)) % 2 == 0 else (20, 20, 20, 255)),
    )


def hard_edge(width: int, height: int):
    return image(width, height, lambda x, _y: (255, 255, 255, 255) if x >= width // 2 else (0, 0, 0, 255))


def color_bars(width: int, height: int):
    bars = [
        (255, 255, 255, 255),
        (255, 255, 0, 255),
        (0, 255, 255, 255),
        (0, 255, 0, 255),
        (255, 0, 255, 255),
        (255, 0, 0, 255),
        (0, 0, 255, 255),
        (0, 0, 0, 255),
    ]
    return image(width, height, lambda x, _y: bars[min(len(bars) - 1, x * len(bars) // width)])


def premult_probe(width: int, height: int):
    den = max(1, width - 1)
    return image(width, height, lambda x, _y: (255, 32, 32, clamp_u8(255 * x / den)))


def numbered_frame(width: int, height: int, frame_index: int):
    r = frame_index & 0xFF
    g = (frame_index >> 8) & 0xFF
    b = (frame_index >> 16) & 0xFF
    pixels = solid(width, height, (r, g, b, 255))
    for bit in range(16):
        if (frame_index >> bit) & 1:
            x0 = bit * width // 16
            x1 = min(width, (bit + 1) * width // 16)
            for y in range(min(height, 20)):
                for x in range(x0, x1):
                    pixels[y * width + x] = (255, 255, 255, 255)
    cx = width // 2
    cy = height // 2
    radius = 12 + (frame_index % 12)
    for y in range(height):
        for x in range(width):
            if abs(math.hypot(x - cx, y - cy) - radius) < 1.1:
                pixels[y * width + x] = (255, 255, 255, 255)
    return pixels


def main() -> None:
    assets = [
        ("transparent.png", solid(SIZE, SIZE, (0, 0, 0, 0)), "fully transparent RGBA"),
        ("impulse_center.png", impulse(SIZE, SIZE), "single-pixel impulse"),
        ("impulse_grid.png", impulse_grid(SIZE, SIZE), "three-pixel impulse grid"),
        ("alpha_square.png", alpha_square(SIZE, SIZE), "hard opaque square on transparent"),
        ("alpha_ramp.png", alpha_ramp(SIZE, SIZE), "white RGB with horizontal alpha ramp"),
        ("luma_ramp.png", luma_ramp(SIZE, SIZE), "horizontal luminance ramp"),
        ("coordinate_field.png", coordinate_field(SIZE, SIZE), "R=x, G=y coordinate field"),
        ("checkerboard_16.png", checkerboard(SIZE, SIZE, 16), "16px checkerboard"),
        ("hard_edge.png", hard_edge(SIZE, SIZE), "black/white hard vertical edge"),
        ("color_bars.png", color_bars(SIZE, SIZE), "RGBA color bars"),
        ("premult_probe.png", premult_probe(SIZE, SIZE), "red alpha ramp for premult audit"),
    ]
    manifest = []
    for filename, pixels, description in assets:
        save_png(OUT / filename, SIZE, SIZE, pixels)
        manifest.append(
            {
                "id": Path(filename).stem,
                "path": f"assets/primitives/{filename}",
                "width": SIZE,
                "height": SIZE,
                "description": description,
            }
        )

    numbered_dir = OUT / "numbered_frames"
    for index in range(NUMBERED_FRAMES):
        save_png(numbered_dir / f"frame_{index:04d}.png", SIZE, SIZE, numbered_frame(SIZE, SIZE, index))
    manifest.append(
        {
            "id": "numbered_frames",
            "path": "assets/primitives/numbered_frames/frame_0000.png",
            "width": SIZE,
            "height": SIZE,
            "frames": NUMBERED_FRAMES,
            "description": "PNG sequence with frame index encoded in RGB and top bit bars",
        }
    )
    (OUT / "assets_manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
