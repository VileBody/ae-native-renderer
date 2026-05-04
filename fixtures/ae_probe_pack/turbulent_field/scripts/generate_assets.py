#!/usr/bin/env python3
"""Generate deterministic primitives for the Turbulent Displace AE probe pack."""

from __future__ import annotations

import json
import struct
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets" / "primitives"
SIZE = 512
GRID = 32


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


def image(fn) -> list[tuple[int, int, int, int]]:
    return [fn(x, y) for y in range(SIZE) for x in range(SIZE)]


def coordinate_field() -> list[tuple[int, int, int, int]]:
    def pixel(x: int, y: int) -> tuple[int, int, int, int]:
        parity = ((x // GRID) + (y // GRID)) & 1
        return (x & 0xFF, y & 0xFF, 255 if parity else 0, 255)

    return image(pixel)


def sparse_impulse_grid() -> list[tuple[int, int, int, int]]:
    def pixel(x: int, y: int) -> tuple[int, int, int, int]:
        if x == SIZE // 2 and y % GRID == 0:
            return (255, 32, 32, 255)
        if y == SIZE // 2 and x % GRID == 0:
            return (32, 255, 32, 255)
        if x % GRID == 0 and y % GRID == 0:
            return (255, 255, 255, 255)
        return (0, 0, 0, 0)

    return image(pixel)


def hard_edge_alpha_ramp() -> list[tuple[int, int, int, int]]:
    def pixel(x: int, y: int) -> tuple[int, int, int, int]:
        vertical = x >= SIZE // 2
        horizontal = y >= SIZE // 2
        r = 255 if vertical else 0
        g = 255 if horizontal else 0
        b = 255 if (vertical ^ horizontal) else 0
        a = clamp_u8(255 * x / max(1, SIZE - 1))
        return (r, g, b, a)

    return image(pixel)


def unique_checkerboard() -> list[tuple[int, int, int, int]]:
    cells = SIZE // GRID

    def pixel(x: int, y: int) -> tuple[int, int, int, int]:
        if x % GRID == 0 or y % GRID == 0:
            return (0, 0, 0, 255)
        cx = x // GRID
        cy = y // GRID
        cell_id = cy * cells + cx
        if ((x // 4) + (y // 4)) & 1:
            shade = 34
        else:
            shade = 0
        r = (37 * cell_id + 31 + shade) & 0xFF
        g = (67 * cell_id + 79 + shade) & 0xFF
        b = (101 * cell_id + 139 + shade) & 0xFF
        return (r, g, b, 255)

    return image(pixel)


def main() -> None:
    assets = [
        (
            "coordinate_field.png",
            coordinate_field(),
            "R=x mod 256, G=y mod 256, B=32px tile parity, A=255.",
        ),
        (
            "sparse_impulse_grid.png",
            sparse_impulse_grid(),
            "One-pixel impulses every 32px with red/green center axes.",
        ),
        (
            "hard_edge_alpha_ramp.png",
            hard_edge_alpha_ramp(),
            "Vertical and horizontal hard edges with a horizontal alpha ramp.",
        ),
        (
            "unique_checkerboard.png",
            unique_checkerboard(),
            "16x16 grid of unique colored 32px cells with 4px intra-cell parity.",
        ),
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

    (OUT / "assets_manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
