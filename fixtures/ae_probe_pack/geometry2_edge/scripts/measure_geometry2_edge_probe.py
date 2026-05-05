#!/usr/bin/env python3
"""Measure AE Geometry2 edge/sampler probe PNGs.

The script has three jobs:

* generate deterministic primitive PNG assets used by the AE JSX builder;
* consume AE-rendered PNG sequences and write JSON/CSV evidence;
* run a local synthetic self-test for the measurement path.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import struct
import tempfile
import zlib
from dataclasses import dataclass
from pathlib import Path
from statistics import mean
from typing import Iterable


SIZE = 64
PACK_ID = "geometry2_edge_sampler_probe_v1"
CHANNELS = ("r", "g", "b", "a")


@dataclass(frozen=True)
class SourceDef:
    id: str
    token: str
    filename: str
    description: str


@dataclass(frozen=True)
class TransformDef:
    id: str
    title: str
    anchor: tuple[float, float]
    position: tuple[float, float]
    scale: tuple[float, float]
    rotation: float


@dataclass(frozen=True)
class SamplingDef:
    id: str
    value: int
    label: str


SOURCES = [
    SourceDef(
        "coord_ramp",
        "COORD",
        "coord_ramp_64.png",
        "R=x*4, G=y*4, B=32/224 parity, A=255.",
    ),
    SourceDef(
        "checker_1px",
        "CHECKER",
        "checker_1px_64.png",
        "1px black/white checker, A=255.",
    ),
    SourceDef(
        "edge_impulse",
        "IMPULSE",
        "edge_impulse_64.png",
        "Single-pixel impulses at corners, center, and edge midpoints.",
    ),
    SourceDef(
        "alpha_hidden_rgb_border",
        "HIDDEN",
        "alpha_hidden_rgb_border_64.png",
        "Transparent 1px border with nonzero hidden RGB; opaque coordinate interior.",
    ),
]

TRANSFORMS = [
    TransformDef("IDENTITY", "identity transform", (32.0, 32.0), (32.0, 32.0), (100.0, 100.0), 0.0),
    TransformDef(
        "SUBPIXEL_RIGHT_0_5",
        "subpixel translate +0.5 output x",
        (32.0, 32.0),
        (32.5, 32.0),
        (100.0, 100.0),
        0.0,
    ),
    TransformDef(
        "SUBPIXEL_LEFT_0_5",
        "subpixel translate -0.5 output x",
        (32.0, 32.0),
        (31.5, 32.0),
        (100.0, 100.0),
        0.0,
    ),
    TransformDef(
        "EDGE_LEFT_1_0",
        "translate -1.0 output x",
        (32.0, 32.0),
        (31.0, 32.0),
        (100.0, 100.0),
        0.0,
    ),
    TransformDef("SCALE_90", "scale 90 percent", (32.0, 32.0), (32.0, 32.0), (90.0, 90.0), 0.0),
    TransformDef(
        "ROTATE_5",
        "small +5 degree rotation",
        (32.0, 32.0),
        (32.0, 32.0),
        (100.0, 100.0),
        5.0,
    ),
]

SAMPLING = [
    SamplingDef("Q1_BILINEAR", 1, "Bilinear"),
    SamplingDef("Q2_BICUBIC", 2, "Bicubic"),
]

SOURCE_BY_TOKEN = {source.token: source for source in SOURCES}
SOURCE_BY_ID = {source.id: source for source in SOURCES}
TRANSFORM_BY_ID = {transform.id: transform for transform in TRANSFORMS}
SAMPLING_BY_ID = {sampling.id: sampling for sampling in SAMPLING}
UV_TARGETS = [-0.5, 0.0, 0.5, 63.0, 63.5, 64.0]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--pack",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="Path to geometry2_edge probe pack root.",
    )
    parser.add_argument(
        "--png-root",
        type=Path,
        default=None,
        help="AE PNG root. Defaults to <pack>/ae_goldens/png8.",
    )
    parser.add_argument(
        "--output-json",
        type=Path,
        default=None,
        help="Output JSON path. Defaults to <pack>/ae_goldens/metadata/geometry2_edge_measurements.json.",
    )
    parser.add_argument(
        "--output-csv",
        type=Path,
        default=None,
        help="Output CSV path. Defaults to <pack>/ae_goldens/metadata/geometry2_edge_samples.csv.",
    )
    parser.add_argument(
        "--generate-assets",
        action="store_true",
        help="Generate primitive PNG assets and assets_manifest.json, then exit unless combined with measurement.",
    )
    parser.add_argument(
        "--measure-after-generate",
        action="store_true",
        help="Continue to measurement after --generate-assets.",
    )
    parser.add_argument(
        "--no-json-samples",
        action="store_true",
        help="Omit per-sample arrays from JSON. CSV is still complete.",
    )
    parser.add_argument("--self-test", action="store_true", help="Run synthetic local self-test and exit.")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    if args.generate_assets:
        manifest_path = generate_assets(args.pack)
        print(f"wrote {manifest_path}")
        if not args.measure_after_generate:
            return 0

    require_pillow()
    png_root = args.png_root or args.pack / "ae_goldens" / "png8"
    output_json = args.output_json or (
        args.pack / "ae_goldens" / "metadata" / "geometry2_edge_measurements.json"
    )
    output_csv = args.output_csv or (
        args.pack / "ae_goldens" / "metadata" / "geometry2_edge_samples.csv"
    )
    report = measure_pack(args.pack, png_root, output_json, output_csv, not args.no_json_samples)
    print(f"wrote {output_json}")
    print(f"wrote {output_csv}")
    print(
        "cases={case_count} measured={measured_case_count} pixel_center={center}".format(
            case_count=report["case_count"],
            measured_case_count=report["measured_case_count"],
            center=report["inferred_pixel_center"]["best_candidate"],
        )
    )
    return 0


def require_pillow() -> None:
    try:
        import PIL.Image  # noqa: F401
    except ImportError as exc:
        raise SystemExit("Pillow is required for measurement: python3 -m pip install Pillow") from exc


def case_id(source: SourceDef, transform: TransformDef, sampling: SamplingDef) -> str:
    return f"G2E_{source.token}_{transform.id}_{sampling.id}"


def expected_cases() -> list[dict[str, object]]:
    cases = []
    for source in SOURCES:
        for transform in TRANSFORMS:
            for sampling in SAMPLING:
                cases.append(
                    {
                        "id": case_id(source, transform, sampling),
                        "source": source,
                        "transform": transform,
                        "sampling": sampling,
                    }
                )
    return cases


def parse_case_id(value: str) -> tuple[SourceDef, TransformDef, SamplingDef] | None:
    if not value.startswith("G2E_"):
        return None
    rest = value[4:]
    for source in SOURCES:
        prefix = source.token + "_"
        if not rest.startswith(prefix):
            continue
        tail = rest[len(prefix) :]
        for sampling in SAMPLING:
            suffix = "_" + sampling.id
            if not tail.endswith(suffix):
                continue
            transform_id = tail[: -len(suffix)]
            transform = TRANSFORM_BY_ID.get(transform_id)
            if transform:
                return source, transform, sampling
    return None


def first_png(case_dir: Path, case: str) -> Path | None:
    exact = sorted(case_dir.glob(f"{case}_*.png"))
    if exact:
        return exact[0]
    any_png = sorted(case_dir.glob("*.png"))
    return any_png[0] if any_png else None


def measure_pack(
    pack: Path,
    png_root: Path,
    output_json: Path,
    output_csv: Path,
    include_json_samples: bool,
) -> dict[str, object]:
    from PIL import Image

    asset_pixels = {source.id: primitive_pixels(source.id) for source in SOURCES}
    sample_rows: list[dict[str, object]] = []
    case_reports: list[dict[str, object]] = []
    center_fit_points: list[dict[str, float]] = []
    oob_rows: list[dict[str, object]] = []
    hidden_rgb_rows: list[dict[str, object]] = []
    images_by_case: dict[str, object] = {}

    for expected in expected_cases():
        cid = str(expected["id"])
        source = expected["source"]
        transform = expected["transform"]
        sampling = expected["sampling"]
        assert isinstance(source, SourceDef)
        assert isinstance(transform, TransformDef)
        assert isinstance(sampling, SamplingDef)
        png = first_png(png_root / cid, cid)
        if png is None:
            case_reports.append(
                {
                    "id": cid,
                    "status": "missing_png",
                    "source_id": source.id,
                    "transform_id": transform.id,
                    "sampling_id": sampling.id,
                    "sampling_value": sampling.value,
                }
            )
            continue

        with Image.open(png) as image:
            rgba = image.convert("RGBA")
            pixels = rgba.load()
            width, height = rgba.size
            images_by_case[cid] = rgba.copy()
            case_samples = []
            bilinear_errors = []
            policy_errors = {
                "whole_sample_transparent_black": [],
                "partial_footprint_transparent": [],
                "clamp_or_edge_extend": [],
            }

            for x, y in sample_points(width, height):
                observed = tuple(int(channel) for channel in pixels[x, y])
                uv_integer = source_uv(transform, x, y, "integer")
                uv_half = source_uv(transform, x, y, "half")
                decoded_uv = decode_coord_ramp(observed) if source.id == "coord_ramp" else None
                source_px = asset_pixels[source.id]
                pred_partial = bilinear_sample(source_px, width, height, uv_integer, "transparent")
                pred_clamp = bilinear_sample(source_px, width, height, uv_integer, "clamp")
                pred_whole = (0, 0, 0, 0)
                is_oob = outside_strict(uv_integer, width, height)
                is_near_edge = near_edge_or_target(uv_integer, width, height)

                if decoded_uv and observed[3] >= 240 and not outside_strict(uv_integer, width, height):
                    center_fit_points.append(
                        {
                            "decoded_x": decoded_uv[0],
                            "decoded_y": decoded_uv[1],
                            "integer_x": uv_integer[0],
                            "integer_y": uv_integer[1],
                            "half_x": uv_half[0],
                            "half_y": uv_half[1],
                        }
                    )

                if observed[3] > 0 or not is_oob:
                    bilinear_errors.append(rgba_mae(observed, pred_partial))

                row = {
                    "case_id": cid,
                    "source_id": source.id,
                    "transform_id": transform.id,
                    "sampling_id": sampling.id,
                    "sampling_value": sampling.value,
                    "sampling_label": sampling.label,
                    "png": str(png),
                    "output_x": x,
                    "output_y": y,
                    "uv_integer_x": round(uv_integer[0], 6),
                    "uv_integer_y": round(uv_integer[1], 6),
                    "uv_half_x": round(uv_half[0], 6),
                    "uv_half_y": round(uv_half[1], 6),
                    "r": observed[0],
                    "g": observed[1],
                    "b": observed[2],
                    "a": observed[3],
                    "decoded_source_x": None if decoded_uv is None else round(decoded_uv[0], 6),
                    "decoded_source_y": None if decoded_uv is None else round(decoded_uv[1], 6),
                    "integer_oob": is_oob,
                    "near_edge_or_target": is_near_edge,
                    "sample_class": classify_sample(observed),
                    "hidden_rgb_marker": is_hidden_rgb_marker(observed),
                    "mae_whole_transparent_black": rgba_mae(observed, pred_whole),
                    "mae_partial_footprint_transparent": rgba_mae(observed, pred_partial),
                    "mae_clamp_or_edge_extend": rgba_mae(observed, pred_clamp),
                }
                sample_rows.append(row)
                case_samples.append(row)

                if is_oob or is_near_edge:
                    oob_rows.append(row)
                    policy_errors["whole_sample_transparent_black"].append(row["mae_whole_transparent_black"])
                    policy_errors["partial_footprint_transparent"].append(
                        row["mae_partial_footprint_transparent"]
                    )
                    policy_errors["clamp_or_edge_extend"].append(row["mae_clamp_or_edge_extend"])
                    if source.id == "alpha_hidden_rgb_border" and is_hidden_rgb_marker(observed):
                        hidden_rgb_rows.append(row)

            case_report: dict[str, object] = {
                "id": cid,
                "status": "ok",
                "source_id": source.id,
                "transform_id": transform.id,
                "sampling_id": sampling.id,
                "sampling_value": sampling.value,
                "sampling_label": sampling.label,
                "png": str(png),
                "size": [width, height],
                "bilinear_prediction_mae": round(mean(bilinear_errors), 6) if bilinear_errors else None,
                "edge_sample_count": sum(1 for row in case_samples if row["near_edge_or_target"]),
                "oob_sample_count": sum(1 for row in case_samples if row["integer_oob"]),
                "transparent_sample_count": sum(1 for row in case_samples if row["a"] == 0),
                "partial_alpha_sample_count": sum(1 for row in case_samples if 0 < int(row["a"]) < 255),
                "policy_error_mae": {
                    key: (round(mean(values), 6) if values else None)
                    for key, values in policy_errors.items()
                },
            }
            if include_json_samples:
                case_report["samples"] = case_samples
            else:
                case_report["sample_count"] = len(case_samples)
            case_reports.append(case_report)

    inferred_center = infer_pixel_center(center_fit_points)
    quality_evidence = compare_quality_pairs(images_by_case)
    oob_candidates = summarize_oob_candidates(oob_rows)
    edge_samples = compact_edge_samples(oob_rows)

    report = {
        "schema": "ae-native-renderer.geometry2-edge-measurements.v1",
        "pack_id": PACK_ID,
        "pack": str(pack),
        "png_root": str(png_root),
        "case_count": len(case_reports),
        "measured_case_count": sum(1 for case in case_reports if case["status"] == "ok"),
        "inferred_pixel_center": inferred_center,
        "oob_policy_candidates": oob_candidates,
        "edge_samples": edge_samples,
        "hidden_rgb_evidence": {
            "marker_sample_count": len(hidden_rgb_rows),
            "interpretation": hidden_rgb_interpretation(hidden_rgb_rows),
            "examples": compact_edge_samples(hidden_rgb_rows[:24]),
        },
        "quality_branch_evidence": quality_evidence,
        "cases": case_reports,
    }

    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_csv.parent.mkdir(parents=True, exist_ok=True)
    output_json.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    write_csv(output_csv, sample_rows)
    return report


def sample_points(width: int, height: int) -> list[tuple[int, int]]:
    xs = {0, 1, 2, 3, 4, width // 2 - 1, width // 2, width // 2 + 1}
    ys = {0, 1, 2, 3, 4, height // 2 - 1, height // 2, height // 2 + 1}
    xs.update({width - 5, width - 4, width - 3, width - 2, width - 1})
    ys.update({height - 5, height - 4, height - 3, height - 2, height - 1})
    for x in range(0, width, 4):
        xs.add(x)
    for y in range(0, height, 4):
        ys.add(y)
    points = {(x, y) for x in xs for y in ys if 0 <= x < width and 0 <= y < height}
    for x in range(width):
        points.add((x, 0))
        points.add((x, height - 1))
        points.add((x, height // 2))
    for y in range(height):
        points.add((0, y))
        points.add((width - 1, y))
        points.add((width // 2, y))
    return sorted(points, key=lambda point: (point[1], point[0]))


def source_uv(transform: TransformDef, x: int, y: int, center_model: str) -> tuple[float, float]:
    ox = float(x)
    oy = float(y)
    if center_model == "half":
        ox += 0.5
        oy += 0.5
    elif center_model != "integer":
        raise ValueError(f"unknown center model: {center_model}")

    ax, ay = transform.anchor
    px, py = transform.position
    sx = transform.scale[0] / 100.0 if abs(transform.scale[0]) > 1e-9 else 1.0
    sy = transform.scale[1] / 100.0 if abs(transform.scale[1]) > 1e-9 else 1.0
    radians = math.radians(transform.rotation)
    cos_v = math.cos(radians)
    sin_v = math.sin(radians)

    dx = ox - px
    dy = oy - py
    unrot_x = cos_v * dx + sin_v * dy
    unrot_y = -sin_v * dx + cos_v * dy
    return (ax + unrot_x / sx, ay + unrot_y / sy)


def decode_coord_ramp(rgba: tuple[int, int, int, int]) -> tuple[float, float] | None:
    if rgba[3] == 0:
        return None
    return (rgba[0] / 4.0, rgba[1] / 4.0)


def outside_strict(uv: tuple[float, float], width: int, height: int) -> bool:
    return uv[0] < 0.0 or uv[1] < 0.0 or uv[0] > width - 1 or uv[1] > height - 1


def near_edge_or_target(uv: tuple[float, float], width: int, height: int) -> bool:
    x, y = uv
    if x < 1.0 or y < 1.0 or x > width - 2 or y > height - 2:
        return True
    return any(abs(x - target) <= 0.08 or abs(y - target) <= 0.08 for target in UV_TARGETS)


def classify_sample(rgba: tuple[int, int, int, int]) -> str:
    if rgba[3] == 0:
        if rgba[:3] == (0, 0, 0):
            return "transparent_black"
        return "transparent_nonzero_rgb"
    if rgba[3] == 255:
        return "opaque"
    return "partial_alpha"


def is_hidden_rgb_marker(rgba: tuple[int, int, int, int]) -> bool:
    r, g, b, a = rgba
    return a == 0 and max(r, g, b) >= 96


def rgba_mae(a: tuple[int, int, int, int], b: tuple[int, int, int, int]) -> float:
    return sum(abs(int(a[i]) - int(b[i])) for i in range(4)) / 4.0


def infer_pixel_center(rows: list[dict[str, float]]) -> dict[str, object]:
    if not rows:
        return {
            "best_candidate": "unknown",
            "reason": "no decodable coord_ramp samples",
            "candidates": {},
        }

    errors = {"integer": [], "half": []}
    for row in rows:
        errors["integer"].append(
            math.hypot(row["decoded_x"] - row["integer_x"], row["decoded_y"] - row["integer_y"])
        )
        errors["half"].append(math.hypot(row["decoded_x"] - row["half_x"], row["decoded_y"] - row["half_y"]))

    candidates = {
        key: {
            "sample_count": len(values),
            "mean_error": round(mean(values), 6),
            "max_error": round(max(values), 6),
        }
        for key, values in errors.items()
    }
    best = min(candidates, key=lambda key: candidates[key]["mean_error"])
    delta = candidates["half"]["mean_error"] - candidates["integer"]["mean_error"]
    return {
        "best_candidate": best,
        "confidence_delta_mean_error_half_minus_integer": round(delta, 6),
        "candidates": candidates,
    }


def summarize_oob_candidates(rows: list[dict[str, object]]) -> dict[str, object]:
    if not rows:
        return {"best_candidate": "unknown", "reason": "no edge/OOB samples"}
    candidates = {
        "whole_sample_transparent_black": "mae_whole_transparent_black",
        "partial_footprint_transparent": "mae_partial_footprint_transparent",
        "clamp_or_edge_extend": "mae_clamp_or_edge_extend",
    }
    scores = {}
    for name, key in candidates.items():
        values = [float(row[key]) for row in rows if row.get(key) is not None]
        scores[name] = {
            "sample_count": len(values),
            "mean_mae": round(mean(values), 6) if values else None,
        }
    best = min(
        (name for name, score in scores.items() if score["mean_mae"] is not None),
        key=lambda name: scores[name]["mean_mae"],
        default="unknown",
    )
    class_counts: dict[str, int] = {}
    for row in rows:
        key = str(row["sample_class"])
        class_counts[key] = class_counts.get(key, 0) + 1
    return {
        "best_candidate": best,
        "candidate_scores": scores,
        "sample_class_counts": class_counts,
    }


def compact_edge_samples(rows: Iterable[dict[str, object]], limit: int = 96) -> list[dict[str, object]]:
    compact = []
    for row in rows:
        compact.append(
            {
                "case_id": row["case_id"],
                "source_id": row["source_id"],
                "transform_id": row["transform_id"],
                "sampling_id": row["sampling_id"],
                "output_xy": [row["output_x"], row["output_y"]],
                "uv_integer": [row["uv_integer_x"], row["uv_integer_y"]],
                "uv_half": [row["uv_half_x"], row["uv_half_y"]],
                "rgba": [row["r"], row["g"], row["b"], row["a"]],
                "sample_class": row["sample_class"],
                "hidden_rgb_marker": row["hidden_rgb_marker"],
            }
        )
        if len(compact) >= limit:
            break
    return compact


def hidden_rgb_interpretation(rows: list[dict[str, object]]) -> str:
    if rows:
        return "AE output contains alpha-zero pixels with nonzero RGB from the hidden-border primitive."
    return "No hidden RGB marker survived in sampled output; interpret transparent-vs-clamp using alpha and clamp candidate scores."


def compare_quality_pairs(images_by_case: dict[str, object]) -> dict[str, object]:
    pairs = []
    for source in SOURCES:
        for transform in TRANSFORMS:
            q1 = case_id(source, transform, SAMPLING[0])
            q2 = case_id(source, transform, SAMPLING[1])
            if q1 not in images_by_case or q2 not in images_by_case:
                continue
            diff = image_diff(images_by_case[q1], images_by_case[q2])
            pairs.append(
                {
                    "source_id": source.id,
                    "transform_id": transform.id,
                    "q1_case_id": q1,
                    "q2_case_id": q2,
                    **diff,
                }
            )
    if not pairs:
        return {
            "distinct_branch_detected": False,
            "reason": "no Q1/Q2 case pairs measured",
            "pairs": [],
        }
    max_mae = max(pair["mean_abs_channel_delta"] for pair in pairs)
    max_pixel_delta = max(pair["max_channel_delta"] for pair in pairs)
    sensitive = [
        pair
        for pair in pairs
        if pair["source_id"] in {"checker_1px", "edge_impulse"} and pair["mean_abs_channel_delta"] > 0.25
    ]
    return {
        "distinct_branch_detected": max_mae > 0.25 or max_pixel_delta > 8,
        "max_mean_abs_channel_delta": round(max_mae, 6),
        "max_channel_delta": max_pixel_delta,
        "sensitive_pair_count": len(sensitive),
        "pairs": pairs,
    }


def image_diff(left, right) -> dict[str, object]:
    if left.size != right.size:
        return {
            "status": "size_mismatch",
            "left_size": list(left.size),
            "right_size": list(right.size),
            "mean_abs_channel_delta": 999.0,
            "max_channel_delta": 999,
            "changed_pixel_count": 0,
        }
    lp = left.load()
    rp = right.load()
    width, height = left.size
    total = 0
    max_delta = 0
    changed = 0
    for y in range(height):
        for x in range(width):
            l = lp[x, y]
            r = rp[x, y]
            pixel_changed = False
            for i in range(4):
                delta = abs(int(l[i]) - int(r[i]))
                total += delta
                max_delta = max(max_delta, delta)
                pixel_changed = pixel_changed or delta != 0
            if pixel_changed:
                changed += 1
    return {
        "status": "ok",
        "mean_abs_channel_delta": round(total / (width * height * 4), 6),
        "max_channel_delta": max_delta,
        "changed_pixel_count": changed,
    }


def write_csv(path: Path, rows: list[dict[str, object]]) -> None:
    fields = [
        "case_id",
        "source_id",
        "transform_id",
        "sampling_id",
        "sampling_value",
        "sampling_label",
        "png",
        "output_x",
        "output_y",
        "uv_integer_x",
        "uv_integer_y",
        "uv_half_x",
        "uv_half_y",
        "r",
        "g",
        "b",
        "a",
        "decoded_source_x",
        "decoded_source_y",
        "integer_oob",
        "near_edge_or_target",
        "sample_class",
        "hidden_rgb_marker",
        "mae_whole_transparent_black",
        "mae_partial_footprint_transparent",
        "mae_clamp_or_edge_extend",
    ]
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        for row in rows:
            writer.writerow({field: row.get(field) for field in fields})


def generate_assets(pack: Path) -> Path:
    out = pack / "assets" / "primitives"
    out.mkdir(parents=True, exist_ok=True)
    manifest = []
    for source in SOURCES:
        pixels = primitive_pixels(source.id)
        save_png(out / source.filename, SIZE, SIZE, pixels)
        manifest.append(
            {
                "id": source.id,
                "path": f"assets/primitives/{source.filename}",
                "width": SIZE,
                "height": SIZE,
                "description": source.description,
            }
        )
    manifest_path = out / "assets_manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return manifest_path


def primitive_pixels(source_id: str) -> list[tuple[int, int, int, int]]:
    if source_id == "coord_ramp":
        return [coord_pixel(x, y) for y in range(SIZE) for x in range(SIZE)]
    if source_id == "checker_1px":
        return [checker_pixel(x, y) for y in range(SIZE) for x in range(SIZE)]
    if source_id == "edge_impulse":
        return [impulse_pixel(x, y) for y in range(SIZE) for x in range(SIZE)]
    if source_id == "alpha_hidden_rgb_border":
        return [hidden_border_pixel(x, y) for y in range(SIZE) for x in range(SIZE)]
    raise ValueError(f"unknown source id: {source_id}")


def coord_pixel(x: int, y: int) -> tuple[int, int, int, int]:
    parity = ((x + y) & 1) == 1
    return (x * 4, y * 4, 224 if parity else 32, 255)


def checker_pixel(x: int, y: int) -> tuple[int, int, int, int]:
    value = 255 if ((x + y) & 1) else 0
    return (value, value, value, 255)


def impulse_pixel(x: int, y: int) -> tuple[int, int, int, int]:
    impulses = {
        (0, 0): (255, 0, 0, 255),
        (63, 0): (0, 255, 0, 255),
        (0, 63): (0, 0, 255, 255),
        (63, 63): (255, 255, 255, 255),
        (32, 32): (255, 64, 64, 255),
        (0, 32): (64, 255, 64, 255),
        (63, 32): (64, 64, 255, 255),
        (32, 0): (255, 255, 64, 255),
        (32, 63): (255, 64, 255, 255),
    }
    return impulses.get((x, y), (0, 0, 0, 0))


def hidden_border_pixel(x: int, y: int) -> tuple[int, int, int, int]:
    if x == 0:
        return (255, 0, 255, 0)
    if y == 0:
        return (0, 255, 255, 0)
    if x == SIZE - 1:
        return (255, 128, 0, 0)
    if y == SIZE - 1:
        return (64, 128, 255, 0)
    return (x * 4, y * 4, 128, 255)


def bilinear_sample(
    pixels: list[tuple[int, int, int, int]],
    width: int,
    height: int,
    uv: tuple[float, float],
    policy: str,
) -> tuple[int, int, int, int]:
    x, y = uv
    if policy == "whole_transparent" and outside_strict(uv, width, height):
        return (0, 0, 0, 0)
    x0 = math.floor(x)
    y0 = math.floor(y)
    tx = x - x0
    ty = y - y0
    accum = [0.0, 0.0, 0.0, 0.0]
    for dy, wy in [(0, 1.0 - ty), (1, ty)]:
        for dx, wx in [(0, 1.0 - tx), (1, tx)]:
            sx = x0 + dx
            sy = y0 + dy
            weight = wx * wy
            if policy == "clamp":
                sx = min(width - 1, max(0, sx))
                sy = min(height - 1, max(0, sy))
                sample = pixels[sy * width + sx]
            elif policy == "transparent":
                if sx < 0 or sy < 0 or sx >= width or sy >= height:
                    sample = (0, 0, 0, 0)
                else:
                    sample = pixels[sy * width + sx]
            else:
                raise ValueError(f"unknown policy: {policy}")
            for channel in range(4):
                accum[channel] += sample[channel] * weight
    return tuple(clamp_u8(value) for value in accum)


def clamp_u8(value: float) -> int:
    return max(0, min(255, int(round(value))))


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
        + png_chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + png_chunk(b"IDAT", zlib.compress(raw, 9))
        + png_chunk(b"IEND", b"")
    )
    path.write_bytes(data)


def png_chunk(kind: bytes, data: bytes) -> bytes:
    return (
        struct.pack(">I", len(data))
        + kind
        + data
        + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
    )


def self_test() -> int:
    require_pillow()
    with tempfile.TemporaryDirectory(prefix="geometry2_edge_selftest_") as tmp:
        pack = Path(tmp)
        generate_assets(pack)
        png_root = pack / "ae_goldens" / "png8"
        render_synthetic_pack(png_root)
        report = measure_pack(
            pack,
            png_root,
            pack / "ae_goldens" / "metadata" / "geometry2_edge_measurements.json",
            pack / "ae_goldens" / "metadata" / "geometry2_edge_samples.csv",
            False,
        )
        center = report["inferred_pixel_center"]["best_candidate"]
        measured = report["measured_case_count"]
        if center != "integer":
            raise SystemExit(f"self-test failed: expected integer pixel center, got {center}")
        if measured != len(expected_cases()):
            raise SystemExit(f"self-test failed: measured {measured}, expected {len(expected_cases())}")
        print("self-test ok")
        print(f"synthetic measured cases={measured} pixel_center={center}")
    return 0


def render_synthetic_pack(png_root: Path) -> None:
    source_pixels = {source.id: primitive_pixels(source.id) for source in SOURCES}
    for expected in expected_cases():
        cid = str(expected["id"])
        source = expected["source"]
        transform = expected["transform"]
        assert isinstance(source, SourceDef)
        assert isinstance(transform, TransformDef)
        pixels = []
        for y in range(SIZE):
            for x in range(SIZE):
                uv = source_uv(transform, x, y, "integer")
                pixels.append(bilinear_sample(source_pixels[source.id], SIZE, SIZE, uv, "transparent"))
        save_png(png_root / cid / f"{cid}_00000.png", SIZE, SIZE, pixels)


if __name__ == "__main__":
    raise SystemExit(main())
