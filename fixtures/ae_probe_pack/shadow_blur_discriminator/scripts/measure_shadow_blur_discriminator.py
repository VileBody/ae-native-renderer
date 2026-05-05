#!/usr/bin/env python3
"""Measure AE Drop Shadow / Box Blur discriminator PNG outputs.

The script reads rendered AE PNG folders, emits JSON + CSV metrics, and can
generate deterministic local assets/self-test PNGs without launching AE.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import random
from dataclasses import dataclass
from pathlib import Path
from statistics import fmean
from typing import Any


WIDTH = 512
HEIGHT = 512
SRC_BBOX = [216, 224, 295, 287]
SRC_CENTER = [255.5, 255.5]
DISTANCE_DIR = 23.0
DISTANCE_SOFT = 19.0
SOFT_DIRECTION = 135.0
NOISE_ASSET = "assets/noisy_rgb_under_alpha.png"


@dataclass(frozen=True)
class CaseMeta:
    id: str
    family: str
    role: str
    direction: float | None = None
    distance: float | None = None
    softness: float | None = None
    shadow_only: int | None = None
    box_radius: float | None = None
    box_iterations: int | None = None
    reference: str = "SHBL_SRC_HARD_ALPHA"


def ae_offset(direction: float, distance: float) -> tuple[float, float]:
    radians = math.radians(direction)
    return (math.sin(radians) * distance, -math.cos(radians) * distance)


def case_meta() -> list[CaseMeta]:
    cases: list[CaseMeta] = [
        CaseMeta("SHBL_SRC_HARD_ALPHA", "source", "hard alpha source", reference=""),
        CaseMeta("SHBL_SRC_RGB_NOISE_ALPHA", "source", "RGB noise under alpha source", reference=""),
        CaseMeta("SHBL_SRC_TRANSLUCENT_COLOR", "source", "colored translucent source", reference=""),
    ]
    for direction in [0, 30, 45, 90, 120, 135, 180, 210, 225, 270, 300, 315]:
        cases.append(
            CaseMeta(
                f"SHBL_DIR_{int(direction):03d}",
                "direction_sweep",
                "softness 0 shadow-only direction probe",
                direction=direction,
                distance=DISTANCE_DIR,
                softness=0,
                shadow_only=1,
            )
        )
    for softness in [0, 1, 2, 4, 8, 12, 18, 32]:
        cases.append(
            CaseMeta(
                f"SHBL_SOFT_{int(softness):03d}",
                "softness_sweep",
                "Drop Shadow softness sweep",
                direction=SOFT_DIRECTION,
                distance=DISTANCE_SOFT,
                softness=softness,
                shadow_only=1,
            )
        )
    for softness in [8, 12, 18]:
        cases.append(
            CaseMeta(
                f"SHBL_BB_S{softness:02d}_R{softness * 1.4:.1f}_I1".replace(".", "P"),
                "box_blur_candidate",
                "Box Blur candidate: radius softness*1.4, 1 iteration",
                direction=SOFT_DIRECTION,
                distance=DISTANCE_SOFT,
                softness=softness,
                box_radius=softness * 1.4,
                box_iterations=1,
            )
        )
        cases.append(
            CaseMeta(
                f"SHBL_BB_S{softness:02d}_R{math.ceil(softness / 2.71):02d}_I3_DIV271",
                "box_blur_candidate",
                "Box Blur candidate: radius ceil(softness/2.71), 3 iterations",
                direction=SOFT_DIRECTION,
                distance=DISTANCE_SOFT,
                softness=softness,
                box_radius=float(math.ceil(softness / 2.71)),
                box_iterations=3,
            )
        )
    cases.extend(
        [
            CaseMeta(
                "SHBL_NOISE_DSH_SHONLY",
                "alpha_only_rgb_noise",
                "Drop Shadow shadow-only on RGB-noise transparent source",
                direction=SOFT_DIRECTION,
                distance=DISTANCE_SOFT,
                softness=12,
                shadow_only=1,
                reference="SHBL_SRC_RGB_NOISE_ALPHA",
            ),
            CaseMeta(
                "SHBL_NOISE_BOX_RGBA",
                "alpha_only_rgb_noise",
                "Box Blur applied directly to RGB-noise transparent source",
                direction=SOFT_DIRECTION,
                distance=0,
                softness=12,
                box_radius=5,
                box_iterations=3,
                reference="SHBL_SRC_RGB_NOISE_ALPHA",
            ),
            CaseMeta(
                "SHBL_COLOR_SHONLY_1",
                "colored_translucent_composite",
                "Colored translucent source, Drop Shadow shadow-only on",
                direction=315,
                distance=18,
                softness=8,
                shadow_only=1,
                reference="SHBL_SRC_TRANSLUCENT_COLOR",
            ),
            CaseMeta(
                "SHBL_COLOR_SHONLY_0",
                "colored_translucent_composite",
                "Colored translucent source, Drop Shadow shadow-only off",
                direction=315,
                distance=18,
                softness=8,
                shadow_only=0,
                reference="SHBL_SRC_TRANSLUCENT_COLOR",
            ),
        ]
    )
    return cases


CASE_META = case_meta()
CASE_BY_ID = {case.id: case for case in CASE_META}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--pack",
        type=Path,
        default=Path("fixtures/ae_probe_pack/shadow_blur_discriminator"),
        help="Path to this probe pack root.",
    )
    parser.add_argument(
        "--png-root",
        type=Path,
        default=None,
        help="Rendered AE PNG root. Defaults to <pack>/ae_goldens/png.",
    )
    parser.add_argument("--json-out", type=Path, default=None)
    parser.add_argument("--csv-out", type=Path, default=None)
    parser.add_argument("--alpha-threshold", type=int, default=1)
    parser.add_argument(
        "--generate-assets",
        action="store_true",
        help="Generate deterministic PNG assets required by the AE builder.",
    )
    parser.add_argument(
        "--self-test-dir",
        type=Path,
        default=None,
        help="Generate synthetic PNGs into this root, then measure them.",
    )
    args = parser.parse_args()

    try:
        from PIL import Image
    except ImportError as exc:
        raise SystemExit("Pillow is required: python3 -m pip install Pillow") from exc

    if args.generate_assets:
        write_assets(args.pack)

    png_root = args.png_root or (args.pack / "ae_goldens" / "png")
    if args.self_test_dir is not None:
        write_self_test_pngs(args.self_test_dir)
        png_root = args.self_test_dir

    json_out = args.json_out or (args.pack / "ae_goldens" / "metadata" / "shadow_blur_discriminator_measurements.json")
    csv_out = args.csv_out or (args.pack / "ae_goldens" / "metadata" / "shadow_blur_discriminator_measurements.csv")

    output = measure_png_root(png_root, args.alpha_threshold)
    json_out.parent.mkdir(parents=True, exist_ok=True)
    csv_out.parent.mkdir(parents=True, exist_ok=True)
    json_out.write_text(json.dumps(output, indent=2) + "\n", encoding="utf-8")
    write_csv(csv_out, output)
    print(f"wrote {json_out}")
    print(f"wrote {csv_out}")
    return 0


def write_assets(pack: Path) -> None:
    from PIL import Image

    path = pack / NOISE_ASSET
    path.parent.mkdir(parents=True, exist_ok=True)
    rng = random.Random(20260505)
    img = Image.new("RGBA", (128, 128), (0, 0, 0, 0))
    pix = img.load()
    for y in range(128):
        for x in range(128):
            r = (rng.randrange(256) + x * 5) & 255
            g = (rng.randrange(256) + y * 7) & 255
            b = (rng.randrange(256) + x * 3 + y * 11) & 255
            a = 0
            if 36 <= x <= 91 and 42 <= y <= 85:
                a = 255
                r, g, b = 255, 255, 255
            elif 28 <= x <= 99 and 34 <= y <= 93:
                a = 64
            pix[x, y] = (r, g, b, a)
    img.save(path)

    manifest = [
        {
            "id": "noisy_rgb_under_alpha",
            "path": NOISE_ASSET,
            "width": 128,
            "height": 128,
            "description": "Deterministic RGB noise in fully/partially transparent pixels plus a hard alpha island.",
        }
    ]
    (path.parent / "assets_manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {path}")


def measure_png_root(png_root: Path, alpha_threshold: int) -> dict[str, Any]:
    images = {}
    cases = []
    for meta in CASE_META:
        png = first_png(png_root, meta.id)
        if png is None:
            cases.append({"id": meta.id, "status": "missing_png"})
            continue
        rgba = open_rgba(png)
        images[meta.id] = rgba
        cases.append(measure_case(meta, png, rgba, images, alpha_threshold))

    pairs = measure_pairs(images)
    return {
        "schema": "ae-native-renderer.shadow-blur-discriminator.measurements.v1",
        "png_root": str(png_root),
        "alpha_threshold": alpha_threshold,
        "source_bbox_nominal": SRC_BBOX,
        "cases": cases,
        "pair_comparisons": pairs,
    }


def measure_case(meta: CaseMeta, png: Path, rgba, images: dict[str, Any], alpha_threshold: int) -> dict[str, Any]:
    bbox = alpha_bbox(rgba, alpha_threshold)
    bbox10 = alpha_bbox(rgba, 10)
    bbox64 = alpha_bbox(rgba, 64)
    bbox128 = alpha_bbox(rgba, 128)
    rgb_bbox = rgb_nonzero_bbox(rgba)
    rgb_bbox10 = rgb_nonzero_bbox(rgba, 10)
    rgb_bbox64 = rgb_nonzero_bbox(rgba, 64)
    rgb_bbox128 = rgb_nonzero_bbox(rgba, 128)
    centroid = alpha_centroid(rgba, alpha_threshold)
    visual_centroid = rgb_centroid(rgba, 1)
    ref_bbox = reference_bbox(meta, images, alpha_threshold)
    ref_rgb_bbox = reference_rgb_bbox(meta, images)
    offset = bbox_center_delta(bbox, ref_bbox)
    visual_offset = bbox_center_delta(rgb_bbox, ref_rgb_bbox)
    spread = spread_radius(bbox, ref_bbox, offset)
    visual_spread = spread_radius(rgb_bbox, ref_rgb_bbox, visual_offset)
    threshold_spread = {
        "alpha_gt_10": spread_radius(bbox10, ref_bbox, offset),
        "alpha_gt_64": spread_radius(bbox64, ref_bbox, offset),
        "alpha_gt_128": spread_radius(bbox128, ref_bbox, offset),
    }
    visual_threshold_spread = {
        "rgb_gt_10": spread_radius(rgb_bbox10, ref_rgb_bbox, visual_offset),
        "rgb_gt_64": spread_radius(rgb_bbox64, ref_rgb_bbox, visual_offset),
        "rgb_gt_128": spread_radius(rgb_bbox128, ref_rgb_bbox, visual_offset),
    }
    expected = None
    if meta.direction is not None and meta.distance is not None:
        dx, dy = ae_offset(meta.direction, meta.distance)
        expected = {"dx": dx, "dy": dy}

    case = {
        "id": meta.id,
        "status": "ok",
        "family": meta.family,
        "role": meta.role,
        "png": str(png),
        "size": list(rgba.size),
        "params": params_dict(meta),
        "alpha_bbox": bbox,
        "alpha_bbox_gt_10": bbox10,
        "alpha_bbox_gt_64": bbox64,
        "alpha_bbox_gt_128": bbox128,
        "rgb_nonzero_bbox": rgb_bbox,
        "rgb_bbox_gt_10": rgb_bbox10,
        "rgb_bbox_gt_64": rgb_bbox64,
        "rgb_bbox_gt_128": rgb_bbox128,
        "bbox_offset_from_reference": offset,
        "rgb_bbox_offset_from_reference": visual_offset,
        "alpha_centroid": centroid,
        "alpha_centroid_offset_from_nominal": centroid_delta(centroid, SRC_CENTER),
        "rgb_centroid": visual_centroid,
        "rgb_centroid_offset_from_nominal": centroid_delta(visual_centroid, SRC_CENTER),
        "expected_ae_projection": expected,
        "alpha_spread_radius": spread,
        "alpha_spread_radius_by_threshold": threshold_spread,
        "rgb_spread_radius": visual_spread,
        "rgb_spread_radius_by_threshold": visual_threshold_spread,
        "channel_checks": channel_checks(rgba, alpha_threshold),
        "edge_profiles": edge_profiles(rgba, bbox, alpha_threshold),
        "visual_edge_profiles": edge_profiles(rgba, rgb_bbox, alpha_threshold),
        "composite_overlap_samples": overlap_samples(meta, rgba),
    }
    return case


def params_dict(meta: CaseMeta) -> dict[str, Any]:
    return {
        key: value
        for key, value in {
            "direction": meta.direction,
            "distance": meta.distance,
            "softness": meta.softness,
            "shadow_only": meta.shadow_only,
            "box_radius": meta.box_radius,
            "box_iterations": meta.box_iterations,
        }.items()
        if value is not None
    }


def first_png(root: Path, case_id: str) -> Path | None:
    case_dir = root / case_id
    candidates = []
    if case_dir.exists():
        candidates.extend(sorted(case_dir.glob("*.png")))
    candidates.extend(sorted(root.glob(f"{case_id}*.png")))
    return candidates[0] if candidates else None


def open_rgba(path: Path):
    from PIL import Image

    with Image.open(path) as image:
        return image.convert("RGBA")


def alpha_bbox(image, threshold: int) -> list[int] | None:
    pix = image.load()
    width, height = image.size
    min_x = width
    min_y = height
    max_x = -1
    max_y = -1
    for y in range(height):
        for x in range(width):
            if pix[x, y][3] > threshold:
                min_x = min(min_x, x)
                min_y = min(min_y, y)
                max_x = max(max_x, x)
                max_y = max(max_y, y)
    return None if max_x < 0 else [min_x, min_y, max_x, max_y]


def rgb_nonzero_bbox(image, threshold: int = 1) -> list[int] | None:
    pix = image.load()
    width, height = image.size
    min_x = width
    min_y = height
    max_x = -1
    max_y = -1
    for y in range(height):
        for x in range(width):
            r, g, b, _a = pix[x, y]
            if max(r, g, b) > threshold:
                min_x = min(min_x, x)
                min_y = min(min_y, y)
                max_x = max(max_x, x)
                max_y = max(max_y, y)
    return None if max_x < 0 else [min_x, min_y, max_x, max_y]


def alpha_centroid(image, threshold: int) -> list[float] | None:
    pix = image.load()
    width, height = image.size
    total = 0.0
    sx = 0.0
    sy = 0.0
    for y in range(height):
        for x in range(width):
            a = pix[x, y][3]
            if a > threshold:
                total += a
                sx += x * a
                sy += y * a
    if total == 0:
        return None
    return [sx / total, sy / total]


def rgb_centroid(image, threshold: int = 1) -> list[float] | None:
    pix = image.load()
    width, height = image.size
    total = 0.0
    sx = 0.0
    sy = 0.0
    for y in range(height):
        for x in range(width):
            r, g, b, _a = pix[x, y]
            weight = max(r, g, b)
            if weight > threshold:
                total += weight
                sx += x * weight
                sy += y * weight
    if total == 0:
        return None
    return [sx / total, sy / total]


def bbox_center_delta(bbox: list[int] | None, ref_bbox: list[int] | None) -> dict[str, float] | None:
    if bbox is None or ref_bbox is None:
        return None
    cx = (bbox[0] + bbox[2]) / 2
    cy = (bbox[1] + bbox[3]) / 2
    rcx = (ref_bbox[0] + ref_bbox[2]) / 2
    rcy = (ref_bbox[1] + ref_bbox[3]) / 2
    return {"dx": cx - rcx, "dy": cy - rcy}


def centroid_delta(centroid: list[float] | None, ref: list[float]) -> dict[str, float] | None:
    if centroid is None:
        return None
    return {"dx": centroid[0] - ref[0], "dy": centroid[1] - ref[1]}


def reference_bbox(meta: CaseMeta, images: dict[str, Any], alpha_threshold: int) -> list[int] | None:
    if meta.reference and meta.reference in images:
        return alpha_bbox(images[meta.reference], alpha_threshold)
    if meta.reference == "":
        return None
    return SRC_BBOX


def reference_rgb_bbox(meta: CaseMeta, images: dict[str, Any]) -> list[int] | None:
    if meta.reference and meta.reference in images:
        return rgb_nonzero_bbox(images[meta.reference])
    if meta.reference == "":
        return None
    return SRC_BBOX


def spread_radius(
    bbox: list[int] | None,
    ref_bbox: list[int] | None,
    offset: dict[str, float] | None,
) -> dict[str, float] | None:
    if bbox is None or ref_bbox is None or offset is None:
        return None
    dx = round(offset["dx"])
    dy = round(offset["dy"])
    shifted = [ref_bbox[0] + dx, ref_bbox[1] + dy, ref_bbox[2] + dx, ref_bbox[3] + dy]
    edges = {
        "left": shifted[0] - bbox[0],
        "top": shifted[1] - bbox[1],
        "right": bbox[2] - shifted[2],
        "bottom": bbox[3] - shifted[3],
    }
    values = [max(0.0, float(v)) for v in edges.values()]
    return {
        **{key: float(value) for key, value in edges.items()},
        "max": max(values),
        "mean": fmean(values),
    }


def channel_checks(image, alpha_threshold: int) -> dict[str, Any]:
    pix = image.load()
    width, height = image.size
    support = []
    transparent_rgb = []
    low_alpha_rgb = []
    for y in range(height):
        for x in range(width):
            r, g, b, a = pix[x, y]
            if a > alpha_threshold:
                support.append((r, g, b, a))
            if a == 0 and max(r, g, b) > 0:
                transparent_rgb.append((r, g, b))
            if 0 < a <= 16:
                low_alpha_rgb.append((r, g, b, a))
    return {
        "alpha_support_pixel_count": len(support),
        "transparent_rgb_nonzero_count": len(transparent_rgb),
        "low_alpha_pixel_count": len(low_alpha_rgb),
        "rgba_stats_alpha_support": stats_rgba(support),
        "rgba_stats_low_alpha": stats_rgba(low_alpha_rgb),
        "rgb_stats_transparent_nonzero": stats_rgb(transparent_rgb),
    }


def stats_rgba(values: list[tuple[int, int, int, int]]) -> dict[str, Any] | None:
    if not values:
        return None
    cols = list(zip(*values))
    return {
        "mean": [fmean(col) for col in cols],
        "min": [min(col) for col in cols],
        "max": [max(col) for col in cols],
        "stddev": [stddev(col) for col in cols],
    }


def stats_rgb(values: list[tuple[int, int, int]]) -> dict[str, Any] | None:
    if not values:
        return None
    cols = list(zip(*values))
    return {
        "mean": [fmean(col) for col in cols],
        "min": [min(col) for col in cols],
        "max": [max(col) for col in cols],
        "stddev": [stddev(col) for col in cols],
    }


def stddev(values) -> float:
    vals = list(values)
    if not vals:
        return 0.0
    mean = fmean(vals)
    return math.sqrt(fmean([(v - mean) * (v - mean) for v in vals]))


def edge_profiles(image, bbox: list[int] | None, alpha_threshold: int) -> dict[str, Any] | None:
    if bbox is None:
        return None
    pix = image.load()
    width, height = image.size
    cx = max(0, min(width - 1, round((bbox[0] + bbox[2]) / 2)))
    cy = max(0, min(height - 1, round((bbox[1] + bbox[3]) / 2)))
    samples = {}
    for name, coords in {
        "horizontal_center": [(x, cy) for x in range(max(0, bbox[0] - 8), min(width - 1, bbox[2] + 8) + 1)],
        "vertical_center": [(cx, y) for y in range(max(0, bbox[1] - 8), min(height - 1, bbox[3] + 8) + 1)],
    }.items():
        sparse = []
        for i, (x, y) in enumerate(coords):
            if i % 4 == 0 or pix[x, y][3] > alpha_threshold:
                sparse.append({"x": x, "y": y, "a": pix[x, y][3], "rgb": list(pix[x, y][:3])})
        samples[name] = sparse
    return samples


def overlap_samples(meta: CaseMeta, image) -> list[dict[str, Any]]:
    if meta.family != "colored_translucent_composite":
        return []
    pix = image.load()
    points = {
        "source_center": (256, 256),
        "shadow_projected_center": (269, 243),
        "source_shadow_overlap": (264, 248),
        "source_left_only": (224, 256),
        "shadow_edge": (298, 236),
    }
    out = []
    for name, (x, y) in points.items():
        out.append({"name": name, "x": x, "y": y, "rgba": list(pix[x, y])})
    return out


def measure_pairs(images: dict[str, Any]) -> list[dict[str, Any]]:
    pairs = []
    for softness in [8, 12, 18]:
        dsh_id = f"SHBL_SOFT_{softness:03d}"
        candidates = [
            f"SHBL_BB_S{softness:02d}_R{softness * 1.4:.1f}_I1".replace(".", "P"),
            f"SHBL_BB_S{softness:02d}_R{math.ceil(softness / 2.71):02d}_I3_DIV271",
        ]
        for candidate_id in candidates:
            if dsh_id in images and candidate_id in images:
                pairs.append(compare_images(dsh_id, candidate_id, images[dsh_id], images[candidate_id]))
    if "SHBL_NOISE_DSH_SHONLY" in images and "SHBL_NOISE_BOX_RGBA" in images:
        pairs.append(compare_images("SHBL_NOISE_DSH_SHONLY", "SHBL_NOISE_BOX_RGBA", images["SHBL_NOISE_DSH_SHONLY"], images["SHBL_NOISE_BOX_RGBA"]))
    return pairs


def compare_images(a_id: str, b_id: str, a_img, b_img) -> dict[str, Any]:
    a_pix = a_img.load()
    b_pix = b_img.load()
    width, height = a_img.size
    sums = [0.0, 0.0, 0.0, 0.0]
    sq = [0.0, 0.0, 0.0, 0.0]
    count = width * height
    for y in range(height):
        for x in range(width):
            av = a_pix[x, y]
            bv = b_pix[x, y]
            for i in range(4):
                diff = abs(av[i] - bv[i])
                sums[i] += diff
                sq[i] += diff * diff
    return {
        "a": a_id,
        "b": b_id,
        "mae_rgba": [value / count for value in sums],
        "rmse_rgba": [math.sqrt(value / count) for value in sq],
    }


def write_csv(path: Path, output: dict[str, Any]) -> None:
    fields = [
        "id",
        "status",
        "family",
        "direction",
        "distance",
        "softness",
        "shadow_only",
        "box_radius",
        "box_iterations",
        "alpha_bbox",
        "bbox_dx",
        "bbox_dy",
        "rgb_bbox",
        "rgb_bbox_dx",
        "rgb_bbox_dy",
        "expected_dx",
        "expected_dy",
        "spread_max",
        "spread_mean",
        "rgb_spread_max",
        "rgb_spread_mean",
        "alpha_pixels",
        "transparent_rgb_nonzero_count",
        "rgb_stddev_support",
        "alpha_stddev_support",
        "png",
    ]
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        for case in output["cases"]:
            params = case.get("params", {})
            offset = case.get("bbox_offset_from_reference") or {}
            rgb_offset = case.get("rgb_bbox_offset_from_reference") or {}
            expected = case.get("expected_ae_projection") or {}
            spread = case.get("alpha_spread_radius") or {}
            rgb_spread = case.get("rgb_spread_radius") or {}
            checks = case.get("channel_checks") or {}
            stats = checks.get("rgba_stats_alpha_support") or {}
            std = stats.get("stddev") or ["", "", "", ""]
            writer.writerow(
                {
                    "id": case["id"],
                    "status": case["status"],
                    "family": case.get("family", ""),
                    "direction": params.get("direction", ""),
                    "distance": params.get("distance", ""),
                    "softness": params.get("softness", ""),
                    "shadow_only": params.get("shadow_only", ""),
                    "box_radius": params.get("box_radius", ""),
                    "box_iterations": params.get("box_iterations", ""),
                    "alpha_bbox": json.dumps(case.get("alpha_bbox")),
                    "bbox_dx": offset.get("dx", ""),
                    "bbox_dy": offset.get("dy", ""),
                    "rgb_bbox": json.dumps(case.get("rgb_nonzero_bbox")),
                    "rgb_bbox_dx": rgb_offset.get("dx", ""),
                    "rgb_bbox_dy": rgb_offset.get("dy", ""),
                    "expected_dx": expected.get("dx", ""),
                    "expected_dy": expected.get("dy", ""),
                    "spread_max": spread.get("max", ""),
                    "spread_mean": spread.get("mean", ""),
                    "rgb_spread_max": rgb_spread.get("max", ""),
                    "rgb_spread_mean": rgb_spread.get("mean", ""),
                    "alpha_pixels": checks.get("alpha_support_pixel_count", ""),
                    "transparent_rgb_nonzero_count": checks.get("transparent_rgb_nonzero_count", ""),
                    "rgb_stddev_support": fmean(std[:3]) if std and std[0] != "" else "",
                    "alpha_stddev_support": std[3] if std and len(std) > 3 else "",
                    "png": case.get("png", ""),
                }
            )


def write_self_test_pngs(root: Path) -> None:
    from PIL import Image, ImageChops, ImageDraw, ImageFilter

    root.mkdir(parents=True, exist_ok=True)

    def save(case_id: str, image) -> None:
        case_dir = root / case_id
        case_dir.mkdir(parents=True, exist_ok=True)
        image.save(case_dir / f"{case_id}_00000.png")

    src = Image.new("RGBA", (WIDTH, HEIGHT), (0, 0, 0, 0))
    draw = ImageDraw.Draw(src)
    draw.rectangle(SRC_BBOX, fill=(255, 255, 255, 255))
    save("SHBL_SRC_HARD_ALPHA", src)

    noise = make_noisy_source_image()
    save("SHBL_SRC_RGB_NOISE_ALPHA", noise)

    color = Image.new("RGBA", (WIDTH, HEIGHT), (0, 0, 0, 0))
    draw = ImageDraw.Draw(color)
    draw.rectangle(SRC_BBOX, fill=(255, 80, 32, 140))
    save("SHBL_SRC_TRANSLUCENT_COLOR", color)

    for meta in CASE_META:
        if meta.family == "direction_sweep":
            dx, dy = ae_offset(meta.direction or 0, meta.distance or 0)
            save(meta.id, shifted_alpha_shadow(src, dx, dy, 0, (255, 255, 255, 180)))
        elif meta.family == "softness_sweep":
            dx, dy = ae_offset(meta.direction or 0, meta.distance or 0)
            save(meta.id, shifted_alpha_shadow(src, dx, dy, meta.softness or 0, (255, 255, 255, 180)))
        elif meta.family == "box_blur_candidate":
            dx, dy = ae_offset(meta.direction or 0, meta.distance or 0)
            shifted = ImageChops.offset(src, round(dx), round(dy))
            blur = shifted
            for _ in range(meta.box_iterations or 1):
                blur = blur.filter(ImageFilter.BoxBlur(meta.box_radius or 0))
            save(meta.id, recolor_alpha(blur, (255, 255, 255), 180))
        elif meta.id == "SHBL_NOISE_DSH_SHONLY":
            dx, dy = ae_offset(meta.direction or 0, meta.distance or 0)
            save(meta.id, shifted_alpha_shadow(noise, dx, dy, meta.softness or 0, (255, 255, 255, 180)))
        elif meta.id == "SHBL_NOISE_BOX_RGBA":
            blur = noise
            for _ in range(meta.box_iterations or 1):
                blur = blur.filter(ImageFilter.BoxBlur(meta.box_radius or 0))
            save(meta.id, blur)
        elif meta.id == "SHBL_COLOR_SHONLY_1":
            dx, dy = ae_offset(meta.direction or 0, meta.distance or 0)
            save(meta.id, shifted_alpha_shadow(color, dx, dy, meta.softness or 0, (20, 140, 255, 130)))
        elif meta.id == "SHBL_COLOR_SHONLY_0":
            dx, dy = ae_offset(meta.direction or 0, meta.distance or 0)
            shadow = shifted_alpha_shadow(color, dx, dy, meta.softness or 0, (20, 140, 255, 130))
            save(meta.id, Image.alpha_composite(shadow, color))


def make_noisy_source_image():
    from PIL import Image

    small = Image.new("RGBA", (128, 128), (0, 0, 0, 0))
    rng = random.Random(20260505)
    pix = small.load()
    for y in range(128):
        for x in range(128):
            r = (rng.randrange(256) + x * 5) & 255
            g = (rng.randrange(256) + y * 7) & 255
            b = (rng.randrange(256) + x * 3 + y * 11) & 255
            a = 0
            if 36 <= x <= 91 and 42 <= y <= 85:
                a = 255
                r, g, b = 255, 255, 255
            elif 28 <= x <= 99 and 34 <= y <= 93:
                a = 64
            pix[x, y] = (r, g, b, a)
    canvas = Image.new("RGBA", (WIDTH, HEIGHT), (0, 0, 0, 0))
    canvas.alpha_composite(small, (192, 192))
    return canvas


def shifted_alpha_shadow(source, dx: float, dy: float, radius: float, color: tuple[int, int, int, int]):
    from PIL import ImageChops, ImageFilter

    shifted = ImageChops.offset(source, round(dx), round(dy))
    if radius > 0:
        shifted = shifted.filter(ImageFilter.BoxBlur(radius / 2.71))
        shifted = shifted.filter(ImageFilter.BoxBlur(radius / 2.71))
        shifted = shifted.filter(ImageFilter.BoxBlur(radius / 2.71))
    return recolor_alpha(shifted, color[:3], color[3])


def recolor_alpha(source, rgb: tuple[int, int, int], opacity: int):
    from PIL import Image

    out = Image.new("RGBA", source.size, (0, 0, 0, 0))
    src = source.load()
    dst = out.load()
    width, height = source.size
    for y in range(height):
        for x in range(width):
            a = int(round(src[x, y][3] * opacity / 255))
            dst[x, y] = (rgb[0], rgb[1], rgb[2], a)
    return out


if __name__ == "__main__":
    raise SystemExit(main())
