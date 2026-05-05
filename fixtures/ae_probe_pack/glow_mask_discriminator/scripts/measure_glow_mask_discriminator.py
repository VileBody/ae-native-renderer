#!/usr/bin/env python3
"""Measure rendered AE Glow mask discriminator probe PNGs.

The script reads PNG sequences rendered by After Effects and writes JSON plus
CSV summaries for mask pass/fail samples, radius profiles, intensity scale, and
RGB/alpha split behavior.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import tempfile
from pathlib import Path
from typing import Any


WIDTH = 512
HEIGHT = 512
CENTER = (256, 256)
MASK_SOURCE_CASE = "GMD_SRC_MASK_SAMPLES"
MASK_CASES = [
    ("GMD_MASK_DEFAULT", "default_absent_0001"),
    ("GMD_MASK_BASEDON_1", "0001=1"),
    ("GMD_MASK_BASEDON_2", "0001=2"),
]
RADIUS_CASES = [
    ("GMD_RADIUS_R0", 0.0),
    ("GMD_RADIUS_R0P5", 0.5),
    ("GMD_RADIUS_R1", 1.0),
    ("GMD_RADIUS_R2", 2.0),
    ("GMD_RADIUS_R5", 5.0),
    ("GMD_RADIUS_R10", 10.0),
]
INTENSITY_CASES = [
    ("GMD_INTENSITY_I0", 0.0),
    ("GMD_INTENSITY_I0P5", 0.5),
    ("GMD_INTENSITY_I1", 1.0),
    ("GMD_INTENSITY_I1P25", 1.25),
    ("GMD_INTENSITY_I2", 2.0),
]
COMPOSITE_CASES = [
    ("GMD_COMP_TRANSPARENT", "transparent"),
    ("GMD_COMP_BLACK", "black"),
    ("GMD_COMP_BLACK_50A", "black_50pct_alpha"),
]
MASK_SAMPLES = [
    {
        "id": "bright_high_alpha",
        "xy": (96, 160),
        "luma_threshold_pass": True,
        "alpha_threshold_pass": True,
    },
    {
        "id": "bright_low_alpha",
        "xy": (200, 160),
        "luma_threshold_pass": True,
        "alpha_threshold_pass": False,
    },
    {
        "id": "dark_high_alpha",
        "xy": (304, 160),
        "luma_threshold_pass": False,
        "alpha_threshold_pass": True,
    },
    {
        "id": "dark_low_alpha",
        "xy": (408, 160),
        "luma_threshold_pass": False,
        "alpha_threshold_pass": False,
    },
    {
        "id": "luma_119_opaque",
        "xy": (200, 320),
        "luma_threshold_pass": False,
        "alpha_threshold_pass": True,
    },
    {
        "id": "luma_120_opaque",
        "xy": (304, 320),
        "luma_threshold_pass": True,
        "alpha_threshold_pass": True,
    },
    {
        "id": "mid_128_high_alpha",
        "xy": (408, 320),
        "luma_threshold_pass": True,
        "alpha_threshold_pass": True,
    },
]
RADIUS_DISTANCES = [0, 1, 2, 3, 4, 5, 6, 8, 10, 12, 16, 20, 24, 32]
INTENSITY_DISTANCES = [4, 5, 6, 8, 10]
COMPOSITE_SAMPLE_POINTS = [
    ("center", (256, 256)),
    ("near_halo_x_plus_5", (261, 256)),
    ("far_halo_x_plus_14", (270, 256)),
    ("background_corner", (32, 32)),
]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--pack",
        type=Path,
        default=Path("fixtures/ae_probe_pack/glow_mask_discriminator"),
        help="Path to the glow_mask_discriminator probe pack root.",
    )
    parser.add_argument(
        "--png-root",
        type=Path,
        default=None,
        help="Directory containing <case_id>/*.png. Defaults to <pack>/ae_probe_outputs/png.",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=None,
        help="Output directory. Defaults to <pack>/measurements.",
    )
    parser.add_argument(
        "--tolerance",
        type=int,
        default=2,
        help="Per-channel delta threshold used to mark mask sample changes.",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Generate temporary dummy PNGs and validate the measurement pipeline.",
    )
    args = parser.parse_args()

    try:
        from PIL import Image
    except ImportError as exc:
        raise SystemExit("Pillow is required: python3 -m pip install Pillow") from exc

    if args.self_test:
        return run_self_test(Image)

    pack = args.pack
    png_root = args.png_root if args.png_root else pack / "ae_probe_outputs" / "png"
    out_dir = args.out_dir if args.out_dir else pack / "measurements"
    result = measure_pack(pack, png_root, out_dir, args.tolerance, Image)
    write_outputs(result, out_dir)
    print(f"wrote {out_dir / 'glow_mask_discriminator_measurements.json'}")
    return 0


def measure_pack(
    pack: Path,
    png_root: Path,
    out_dir: Path,
    tolerance: int,
    image_module: Any,
) -> dict[str, Any]:
    images: dict[str, Any | None] = {}
    case_status: list[dict[str, Any]] = []
    for case_id in all_case_ids():
        png = first_png(png_root / case_id)
        if png is None:
            images[case_id] = None
            case_status.append({"id": case_id, "status": "missing_png"})
            continue
        with image_module.open(png) as image:
            rgba = image.convert("RGBA")
            images[case_id] = rgba.copy()
        case_status.append(
            {
                "id": case_id,
                "status": "ok",
                "png": str(png),
                "size": list(images[case_id].size),
            }
        )

    source = images.get(MASK_SOURCE_CASE)
    mask_samples = measure_mask_samples(source, images, tolerance)
    radius_profile = measure_radius_profile(images)
    intensity_scale = measure_intensity_scale(images)
    composite_samples = measure_composite_samples(images)
    rgb_alpha_split = build_rgb_alpha_split(mask_samples, radius_profile, intensity_scale, composite_samples)

    return {
        "pack": str(pack),
        "png_root": str(png_root),
        "output_dir": str(out_dir),
        "tolerance": tolerance,
        "case_status": case_status,
        "sample_points": {
            "mask_samples": [
                {
                    "id": sample["id"],
                    "xy": list(sample["xy"]),
                    "luma_threshold_pass": sample["luma_threshold_pass"],
                    "alpha_threshold_pass": sample["alpha_threshold_pass"],
                }
                for sample in MASK_SAMPLES
            ],
            "impulse_center": list(CENTER),
            "radius_distances_px": RADIUS_DISTANCES,
            "intensity_distances_px": INTENSITY_DISTANCES,
        },
        "mask_pass_fail_samples": mask_samples,
        "radius_profile": radius_profile,
        "intensity_scale": intensity_scale,
        "rgb_alpha_split": rgb_alpha_split,
        "composite_discriminator": composite_samples,
    }


def measure_mask_samples(source: Any | None, images: dict[str, Any | None], tolerance: int) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    if source is None:
        for case_id, based_on in MASK_CASES:
            rows.append({"case_id": case_id, "based_on": based_on, "status": "missing_source_png"})
        return rows

    for case_id, based_on in MASK_CASES:
        image = images.get(case_id)
        if image is None:
            rows.append({"case_id": case_id, "based_on": based_on, "status": "missing_png"})
            continue
        for sample in MASK_SAMPLES:
            x, y = sample["xy"]
            source_rgba = rgba_at(source, x, y)
            output_rgba = rgba_at(image, x, y)
            delta = [output_rgba[i] - source_rgba[i] for i in range(4)]
            abs_delta = [abs(value) for value in delta]
            rows.append(
                {
                    "case_id": case_id,
                    "based_on": based_on,
                    "status": "ok",
                    "sample_id": sample["id"],
                    "x": x,
                    "y": y,
                    "source_rgba": source_rgba,
                    "output_rgba": output_rgba,
                    "delta_rgba": delta,
                    "rgb_delta_luma": round(rgb_luma(output_rgba) - rgb_luma(source_rgba), 4),
                    "alpha_delta": delta[3],
                    "changed": max(abs_delta) > tolerance,
                    "luma_threshold_pass": sample["luma_threshold_pass"],
                    "alpha_threshold_pass": sample["alpha_threshold_pass"],
                }
            )
    return rows


def measure_radius_profile(images: dict[str, Any | None]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    cx, cy = CENTER
    for case_id, radius in RADIUS_CASES:
        image = images.get(case_id)
        if image is None:
            rows.append({"case_id": case_id, "radius": radius, "status": "missing_png"})
            continue
        for distance in RADIUS_DISTANCES:
            for direction, sign in (("left", -1), ("right", 1)):
                x = cx + sign * distance
                rgba = rgba_at(image, x, cy)
                rows.append(
                    {
                        "case_id": case_id,
                        "radius": radius,
                        "status": "ok",
                        "direction": direction,
                        "distance_px": distance,
                        "x": x,
                        "y": cy,
                        "rgba": rgba,
                        "rgb_luma": round(rgb_luma(rgba), 4),
                        "alpha": rgba[3],
                    }
                )
    return rows


def measure_intensity_scale(images: dict[str, Any | None]) -> list[dict[str, Any]]:
    raw_rows: list[dict[str, Any]] = []
    cx, cy = CENTER
    for case_id, intensity in INTENSITY_CASES:
        image = images.get(case_id)
        if image is None:
            raw_rows.append({"case_id": case_id, "intensity": intensity, "status": "missing_png"})
            continue
        samples = []
        for distance in INTENSITY_DISTANCES:
            for x, y in ring_points(cx, cy, distance):
                rgba = rgba_at(image, x, y)
                samples.append(
                    {
                        "distance_px": distance,
                        "x": x,
                        "y": y,
                        "rgba": rgba,
                        "rgb_luma": rgb_luma(rgba),
                        "alpha": rgba[3],
                    }
                )
        luma_mean = mean([sample["rgb_luma"] for sample in samples])
        alpha_mean = mean([sample["alpha"] for sample in samples])
        raw_rows.append(
            {
                "case_id": case_id,
                "intensity": intensity,
                "status": "ok",
                "radius": 5,
                "sample_count": len(samples),
                "ring_luma_mean": round(luma_mean, 4),
                "ring_alpha_mean": round(alpha_mean, 4),
                "center_rgba": rgba_at(image, cx, cy),
                "samples": [
                    {
                        **sample,
                        "rgb_luma": round(sample["rgb_luma"], 4),
                    }
                    for sample in samples
                ],
            }
        )

    base_luma = next(
        (row["ring_luma_mean"] for row in raw_rows if row.get("case_id") == "GMD_INTENSITY_I1" and row.get("status") == "ok"),
        None,
    )
    base_alpha = next(
        (row["ring_alpha_mean"] for row in raw_rows if row.get("case_id") == "GMD_INTENSITY_I1" and row.get("status") == "ok"),
        None,
    )
    for row in raw_rows:
        if row.get("status") != "ok":
            continue
        row["relative_luma_to_i1"] = safe_ratio(row["ring_luma_mean"], base_luma)
        row["relative_alpha_to_i1"] = safe_ratio(row["ring_alpha_mean"], base_alpha)
    return raw_rows


def measure_composite_samples(images: dict[str, Any | None]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for case_id, background in COMPOSITE_CASES:
        image = images.get(case_id)
        if image is None:
            rows.append({"case_id": case_id, "background": background, "status": "missing_png"})
            continue
        for sample_id, (x, y) in COMPOSITE_SAMPLE_POINTS:
            rgba = rgba_at(image, x, y)
            rows.append(
                {
                    "case_id": case_id,
                    "background": background,
                    "status": "ok",
                    "sample_id": sample_id,
                    "x": x,
                    "y": y,
                    "rgba": rgba,
                    "rgb_luma": round(rgb_luma(rgba), 4),
                    "alpha": rgba[3],
                }
            )
    return rows


def build_rgb_alpha_split(
    mask_samples: list[dict[str, Any]],
    radius_profile: list[dict[str, Any]],
    intensity_scale: list[dict[str, Any]],
    composite_samples: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for row in mask_samples:
        if row.get("status") != "ok":
            continue
        rows.append(
            {
                "group": "mask",
                "case_id": row["case_id"],
                "sample_id": row["sample_id"],
                "x": row["x"],
                "y": row["y"],
                "rgb_luma": round(rgb_luma(row["output_rgba"]), 4),
                "alpha": row["output_rgba"][3],
                "source_rgb_luma": round(rgb_luma(row["source_rgba"]), 4),
                "source_alpha": row["source_rgba"][3],
                "rgb_delta_luma": row["rgb_delta_luma"],
                "alpha_delta": row["alpha_delta"],
            }
        )
    for row in radius_profile:
        if row.get("status") == "ok" and row.get("distance_px") in (0, 5, 10, 20):
            rows.append(
                {
                    "group": "radius",
                    "case_id": row["case_id"],
                    "sample_id": f"{row['direction']}_{row['distance_px']}",
                    "x": row["x"],
                    "y": row["y"],
                    "rgb_luma": row["rgb_luma"],
                    "alpha": row["alpha"],
                }
            )
    for row in intensity_scale:
        if row.get("status") == "ok":
            rows.append(
                {
                    "group": "intensity",
                    "case_id": row["case_id"],
                    "sample_id": "ring_mean",
                    "x": CENTER[0],
                    "y": CENTER[1],
                    "rgb_luma": row["ring_luma_mean"],
                    "alpha": row["ring_alpha_mean"],
                    "relative_luma_to_i1": row.get("relative_luma_to_i1"),
                    "relative_alpha_to_i1": row.get("relative_alpha_to_i1"),
                }
            )
    for row in composite_samples:
        if row.get("status") == "ok":
            rows.append(
                {
                    "group": "composite",
                    "case_id": row["case_id"],
                    "sample_id": row["sample_id"],
                    "x": row["x"],
                    "y": row["y"],
                    "rgb_luma": row["rgb_luma"],
                    "alpha": row["alpha"],
                    "background": row["background"],
                }
            )
    return rows


def write_outputs(result: dict[str, Any], out_dir: Path) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "glow_mask_discriminator_measurements.json").write_text(
        json.dumps(result, indent=2),
        encoding="utf-8",
    )
    write_csv(out_dir / "mask_samples.csv", flatten_rows(result["mask_pass_fail_samples"]))
    write_csv(out_dir / "radius_profile.csv", flatten_rows(result["radius_profile"]))
    write_csv(out_dir / "intensity_scale.csv", flatten_rows(result["intensity_scale"], drop_keys={"samples"}))
    write_csv(out_dir / "rgb_alpha_split.csv", flatten_rows(result["rgb_alpha_split"]))
    write_csv(out_dir / "composite_samples.csv", flatten_rows(result["composite_discriminator"]))


def write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    if not rows:
        path.write_text("", encoding="utf-8")
        return
    fields: list[str] = []
    for row in rows:
        for key in row:
            if key not in fields:
                fields.append(key)
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        for row in rows:
            writer.writerow({key: csv_value(row.get(key)) for key in fields})


def flatten_rows(rows: list[dict[str, Any]], drop_keys: set[str] | None = None) -> list[dict[str, Any]]:
    drop_keys = drop_keys or set()
    flattened = []
    for row in rows:
        flat = {}
        for key, value in row.items():
            if key in drop_keys:
                continue
            if isinstance(value, dict):
                for child_key, child_value in value.items():
                    flat[f"{key}_{child_key}"] = child_value
            else:
                flat[key] = value
        flattened.append(flat)
    return flattened


def first_png(case_dir: Path) -> Path | None:
    files = sorted(case_dir.glob("*.png"))
    return files[0] if files else None


def all_case_ids() -> list[str]:
    return (
        [MASK_SOURCE_CASE]
        + [case_id for case_id, _ in MASK_CASES]
        + [case_id for case_id, _ in RADIUS_CASES]
        + [case_id for case_id, _ in INTENSITY_CASES]
        + [case_id for case_id, _ in COMPOSITE_CASES]
    )


def rgba_at(image: Any, x: int, y: int) -> list[int]:
    width, height = image.size
    if x < 0 or x >= width or y < 0 or y >= height:
        raise ValueError(f"sample ({x}, {y}) outside image size {width}x{height}")
    return list(image.getpixel((x, y)))


def rgb_luma(rgba: list[int]) -> float:
    return 0.2126 * rgba[0] + 0.7152 * rgba[1] + 0.0722 * rgba[2]


def mean(values: list[float]) -> float:
    return sum(values) / len(values) if values else 0.0


def safe_ratio(value: float | None, base: float | None) -> float | None:
    if value is None or base is None or abs(base) < 1e-9:
        return None
    return round(value / base, 6)


def ring_points(cx: int, cy: int, distance: int) -> list[tuple[int, int]]:
    return [
        (cx + distance, cy),
        (cx - distance, cy),
        (cx, cy + distance),
        (cx, cy - distance),
    ]


def csv_value(value: Any) -> Any:
    if isinstance(value, (list, dict)):
        return json.dumps(value, separators=(",", ":"))
    return value


def run_self_test(image_module: Any) -> int:
    with tempfile.TemporaryDirectory(prefix="gmd_probe_selftest_") as tmp:
        pack = Path(tmp) / "pack"
        png_root = pack / "ae_probe_outputs" / "png"
        out_dir = pack / "measurements"
        write_dummy_pngs(png_root, image_module)
        result = measure_pack(pack, png_root, out_dir, tolerance=2, image_module=image_module)
        write_outputs(result, out_dir)
        ok_mask = [row for row in result["mask_pass_fail_samples"] if row.get("status") == "ok"]
        ok_radius = [row for row in result["radius_profile"] if row.get("status") == "ok"]
        ok_intensity = [row for row in result["intensity_scale"] if row.get("status") == "ok"]
        ok_composite = [row for row in result["composite_discriminator"] if row.get("status") == "ok"]
        assert len(ok_mask) == len(MASK_CASES) * len(MASK_SAMPLES)
        assert len(ok_radius) == len(RADIUS_CASES) * len(RADIUS_DISTANCES) * 2
        assert len(ok_intensity) == len(INTENSITY_CASES)
        assert len(ok_composite) == len(COMPOSITE_CASES) * len(COMPOSITE_SAMPLE_POINTS)
        print("self-test passed")
    return 0


def write_dummy_pngs(png_root: Path, image_module: Any) -> None:
    for case_id in all_case_ids():
        case_dir = png_root / case_id
        case_dir.mkdir(parents=True, exist_ok=True)
        image = image_module.new("RGBA", (WIDTH, HEIGHT), (0, 0, 0, 0))
        if case_id == MASK_SOURCE_CASE:
            paint_mask_source(image)
        elif case_id in {case for case, _ in MASK_CASES}:
            paint_mask_source(image)
            paint_dummy_mask_glow(image, case_id)
        elif case_id.startswith("GMD_RADIUS_"):
            paint_dummy_impulse_profile(image, radius_for_case(case_id))
        elif case_id.startswith("GMD_INTENSITY_"):
            paint_dummy_impulse_profile(image, 5.0, intensity_for_case(case_id))
        elif case_id == "GMD_COMP_TRANSPARENT":
            paint_dummy_impulse_profile(image, 10.0, 1.0)
        elif case_id == "GMD_COMP_BLACK":
            paint_background(image, (0, 0, 0, 255))
            paint_dummy_impulse_profile(image, 10.0, 1.0)
        elif case_id == "GMD_COMP_BLACK_50A":
            paint_background(image, (0, 0, 0, 128))
            paint_dummy_impulse_profile(image, 10.0, 1.0)
        image.save(case_dir / f"{case_id}_00000.png")


def paint_mask_source(image: Any) -> None:
    pixels = image.load()
    values = {
        "bright_high_alpha": (190, 190, 190, 255),
        "bright_low_alpha": (190, 190, 190, 89),
        "dark_high_alpha": (41, 41, 41, 255),
        "dark_low_alpha": (41, 41, 41, 89),
        "luma_119_opaque": (119, 119, 119, 255),
        "luma_120_opaque": (120, 120, 120, 255),
        "mid_128_high_alpha": (128, 128, 128, 255),
    }
    for sample in MASK_SAMPLES:
        cx, cy = sample["xy"]
        rgba = values[sample["id"]]
        for y in range(cy - 4, cy + 5):
            for x in range(cx - 4, cx + 5):
                pixels[x, y] = rgba


def paint_dummy_mask_glow(image: Any, case_id: str) -> None:
    pixels = image.load()
    for sample in MASK_SAMPLES:
        use_luma = case_id in {"GMD_MASK_DEFAULT", "GMD_MASK_BASEDON_1"}
        passed = sample["luma_threshold_pass"] if use_luma else sample["alpha_threshold_pass"]
        if not passed:
            continue
        cx, cy = sample["xy"]
        source = pixels[cx, cy]
        pixels[cx, cy] = (
            min(255, source[0] + 40),
            min(255, source[1] + 40),
            min(255, source[2] + 40),
            source[3],
        )


def paint_background(image: Any, rgba: tuple[int, int, int, int]) -> None:
    pixels = image.load()
    for y in range(HEIGHT):
        for x in range(WIDTH):
            pixels[x, y] = rgba


def paint_dummy_impulse_profile(image: Any, radius: float, intensity: float = 1.0) -> None:
    pixels = image.load()
    cx, cy = CENTER
    spread = max(radius, 0.5)
    reach = max(1, int(math.ceil(spread * 4)))
    for y in range(cy - reach, cy + reach + 1):
        for x in range(cx - reach, cx + reach + 1):
            dist = math.hypot(x - cx, y - cy)
            if dist > reach:
                continue
            if radius <= 0 and dist > 0:
                continue
            value = int(max(0, 255 * intensity * math.exp(-(dist * dist) / (2 * spread * spread))))
            old = pixels[x, y]
            pixels[x, y] = (
                min(255, old[0] + value),
                min(255, old[1] + value),
                min(255, old[2] + value),
                min(255, max(old[3], value)),
            )


def radius_for_case(case_id: str) -> float:
    return dict(RADIUS_CASES)[case_id]


def intensity_for_case(case_id: str) -> float:
    return dict(INTENSITY_CASES)[case_id]


if __name__ == "__main__":
    raise SystemExit(main())
