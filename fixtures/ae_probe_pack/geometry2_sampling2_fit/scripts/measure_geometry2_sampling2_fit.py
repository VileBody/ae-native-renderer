#!/usr/bin/env python3
"""Generate and fit AE Geometry2 Sampling=2 probe outputs."""

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
from typing import Callable, Iterable


SIZE = 64
PACK_ID = "geometry2_sampling2_fit_probe_v1"
CHANNELS = 4


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


@dataclass(frozen=True)
class Candidate:
    id: str
    sampler: str
    edge: str
    round_mode: str
    params: dict[str, float]
    sample: Callable[[list[tuple[int, int, int, int]], int, int, tuple[float, float], str, str], tuple[int, int, int, int]]


SOURCES = [
    SourceDef(
        "fit_texture",
        "FIT_TEXTURE",
        "fit_texture_64.png",
        "Opaque deterministic high-frequency RGBA texture for cubic fitting.",
    ),
    SourceDef(
        "impulse_grid",
        "IMPULSE_GRID",
        "impulse_grid_64.png",
        "Transparent canvas with colored impulses every 8 px and at edges.",
    ),
    SourceDef(
        "alpha_steps",
        "ALPHA_STEPS",
        "alpha_steps_64.png",
        "Straight-RGBA texture with deterministic RGB and stepped alpha.",
    ),
]

TRANSFORMS = [
    TransformDef("IDENTITY", "identity", (32.0, 32.0), (32.0, 32.0), (100.0, 100.0), 0.0),
    TransformDef("SHIFT_X_0_125", "source x minus 0.125", (32.0, 32.0), (32.125, 32.0), (100.0, 100.0), 0.0),
    TransformDef("SHIFT_X_0_250", "source x minus 0.250", (32.0, 32.0), (32.250, 32.0), (100.0, 100.0), 0.0),
    TransformDef("SHIFT_X_0_375", "source x minus 0.375", (32.0, 32.0), (32.375, 32.0), (100.0, 100.0), 0.0),
    TransformDef("SHIFT_X_0_500", "source x minus 0.500", (32.0, 32.0), (32.500, 32.0), (100.0, 100.0), 0.0),
    TransformDef("SHIFT_X_0_625", "source x minus 0.625", (32.0, 32.0), (32.625, 32.0), (100.0, 100.0), 0.0),
    TransformDef("SHIFT_X_0_750", "source x minus 0.750", (32.0, 32.0), (32.750, 32.0), (100.0, 100.0), 0.0),
    TransformDef("SHIFT_X_0_875", "source x minus 0.875", (32.0, 32.0), (32.875, 32.0), (100.0, 100.0), 0.0),
    TransformDef("SHIFT_Y_0_250", "source y minus 0.250", (32.0, 32.0), (32.0, 32.250), (100.0, 100.0), 0.0),
    TransformDef(
        "SHIFT_XY_0_250_0_625",
        "source xy mixed",
        (32.0, 32.0),
        (32.250, 32.625),
        (100.0, 100.0),
        0.0,
    ),
    TransformDef("EDGE_LEFT_1_250", "left edge source x minus 1.250", (32.0, 32.0), (33.250, 32.0), (100.0, 100.0), 0.0),
    TransformDef("EDGE_RIGHT_1_250", "right edge source x plus 1.250", (32.0, 32.0), (30.750, 32.0), (100.0, 100.0), 0.0),
    TransformDef("SCALE_92", "scale 92 percent", (32.0, 32.0), (32.0, 32.0), (92.0, 92.0), 0.0),
    TransformDef("ROTATE_3", "rotate 3 degrees", (32.0, 32.0), (32.0, 32.0), (100.0, 100.0), 3.0),
]

SAMPLING = [
    SamplingDef("Q1_BILINEAR", 1, "Bilinear"),
    SamplingDef("Q2_BICUBIC", 2, "Bicubic"),
]

SOURCE_BY_TOKEN = {source.token: source for source in SOURCES}
TRANSFORM_BY_ID = {transform.id: transform for transform in TRANSFORMS}
SAMPLING_BY_ID = {sampling.id: sampling for sampling in SAMPLING}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pack", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--png-root", type=Path, default=None)
    parser.add_argument("--out-dir", type=Path, default=None)
    parser.add_argument("--generate-assets", action="store_true")
    parser.add_argument("--measure-after-generate", action="store_true")
    parser.add_argument("--write-diffs", type=int, default=16)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    if args.generate_assets:
        manifest = generate_assets(args.pack)
        print(f"wrote {manifest}")
        if not args.measure_after_generate:
            return 0

    require_pillow()
    png_root = args.png_root or args.pack / "ae_goldens" / "png8"
    out_dir = args.out_dir or args.pack / "ae_goldens" / "fit_reports"
    report = fit_pack(args.pack, png_root, out_dir, write_diffs=args.write_diffs)
    print(f"wrote {out_dir / 'fit_report.json'}")
    print(
        "measured={measured} q1_mae={q1:.6f} winner={winner} q2_mae={q2:.6f}".format(
            measured=report["measured_case_count"],
            q1=report["q1_bilinear_sanity"]["mean_abs_channel_delta"],
            winner=report["q2_fit"]["winner"]["id"],
            q2=report["q2_fit"]["winner"]["mean_abs_channel_delta"],
        )
    )
    return 0


def require_pillow() -> None:
    try:
        import PIL.Image  # noqa: F401
    except ImportError as exc:
        raise SystemExit("Pillow is required: python3 -m pip install Pillow") from exc


def case_id(source: SourceDef, transform: TransformDef, sampling: SamplingDef) -> str:
    return f"G2S_{source.token}_{transform.id}_{sampling.id}"


def expected_cases() -> list[dict[str, object]]:
    out = []
    for source in SOURCES:
        for transform in TRANSFORMS:
            for sampling in SAMPLING:
                out.append({"id": case_id(source, transform, sampling), "source": source, "transform": transform, "sampling": sampling})
    return out


def first_png(case_dir: Path, case: str) -> Path | None:
    exact = sorted(case_dir.glob(f"{case}_*.png"))
    if exact:
        return exact[0]
    any_png = sorted(case_dir.glob("*.png"))
    return any_png[0] if any_png else None


def fit_pack(pack: Path, png_root: Path, out_dir: Path, *, write_diffs: int) -> dict[str, object]:
    from PIL import Image

    source_pixels = {source.id: primitive_pixels(source.id) for source in SOURCES}
    images: dict[str, object] = {}
    image_meta: dict[str, dict[str, object]] = {}
    case_rows: list[dict[str, object]] = []
    missing = []
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
            missing.append(cid)
            continue
        with Image.open(png) as im:
            bands = im.getbands()
            image_meta[cid] = {
                "mode": im.mode,
                "bands": list(bands),
                "has_alpha_band": "A" in bands,
            }
            images[cid] = im.convert("RGBA").copy()
        case_rows.append(
            {
                "case_id": cid,
                "source_id": source.id,
                "transform_id": transform.id,
                "sampling_id": sampling.id,
                "sampling_value": sampling.value,
                "png": str(png),
            }
        )

    candidates = build_candidates()
    q1_cases = [row for row in case_rows if row["sampling_id"] == "Q1_BILINEAR"]
    q2_cases = [row for row in case_rows if row["sampling_id"] == "Q2_BICUBIC"]
    opaque_q1_cases = [row for row in q1_cases if row["source_id"] == "fit_texture"]
    opaque_q2_cases = [row for row in q2_cases if row["source_id"] == "fit_texture"]
    alpha_q1_cases = [row for row in q1_cases if row["source_id"] == "alpha_steps"]
    alpha_q2_cases = [row for row in q2_cases if row["source_id"] == "alpha_steps"]
    q1_sanity = score_cases(
        q1_cases,
        images,
        source_pixels,
        Candidate("bilinear_transparent_round", "bilinear", "transparent", "round", {}, bilinear_sample),
        sample_step=2,
    )
    candidate_scores = [
        score_cases(q2_cases, images, source_pixels, candidate, sample_step=2) for candidate in candidates
    ]
    candidate_scores.sort(key=lambda item: (item["mean_abs_channel_delta"], item["max_channel_delta"], item["id"]))
    top_candidate_ids = {score["id"] for score in candidate_scores[:16]}
    refined_scores = [
        score_cases(q2_cases, images, source_pixels, candidate, sample_step=1)
        for candidate in candidates
        if candidate.id in top_candidate_ids
    ]
    refined_scores.sort(key=lambda item: (item["mean_abs_channel_delta"], item["max_channel_delta"], item["id"]))
    winner = refined_scores[0] if refined_scores else None
    current = next(
        (
            score
            for score in candidate_scores
            if score["id"] == "keys_a_-0.500_transparent_round"
        ),
        None,
    )
    opaque_q1_sanity = score_cases(
        opaque_q1_cases,
        images,
        source_pixels,
        Candidate("bilinear_transparent_round", "bilinear", "transparent", "round", {}, bilinear_sample),
        sample_step=1,
    )
    opaque_scores = [
        score_cases(opaque_q2_cases, images, source_pixels, candidate, sample_step=1)
        for candidate in candidates
    ]
    opaque_scores.sort(key=lambda item: (item["mean_abs_channel_delta"], item["max_channel_delta"], item["id"]))
    opaque_current = next(
        (
            score
            for score in opaque_scores
            if score["id"] == "keys_a_-0.500_transparent_round"
        ),
        None,
    )
    alpha_q1_scores = [
        score_cases(alpha_q1_cases, images, source_pixels, candidate, sample_step=1)
        for candidate in build_alpha_candidates("bilinear")
    ]
    alpha_q1_scores.sort(key=lambda item: (item["mean_abs_channel_delta"], item["max_channel_delta"], item["id"]))
    alpha_q2_scores = [
        score_cases(alpha_q2_cases, images, source_pixels, candidate, sample_step=1)
        for candidate in build_alpha_candidates("bicubic")
    ]
    alpha_q2_scores.sort(key=lambda item: (item["mean_abs_channel_delta"], item["max_channel_delta"], item["id"]))
    alpha_q1_rgb_projection_scores = [
        score_cases_rgb_over_black(alpha_q1_cases, images, source_pixels, candidate, sample_step=1)
        for candidate in build_alpha_candidates("bilinear")
    ]
    alpha_q1_rgb_projection_scores.sort(
        key=lambda item: (item["mean_abs_channel_delta"], item["max_channel_delta"], item["id"])
    )
    alpha_q2_rgb_projection_scores = [
        score_cases_rgb_over_black(alpha_q2_cases, images, source_pixels, candidate, sample_step=1)
        for candidate in build_alpha_candidates("bicubic")
    ]
    alpha_q2_rgb_projection_scores.sort(
        key=lambda item: (item["mean_abs_channel_delta"], item["max_channel_delta"], item["id"])
    )

    out_dir.mkdir(parents=True, exist_ok=True)
    write_candidate_csv(out_dir / "fit_candidates.csv", candidate_scores)
    write_candidate_csv(out_dir / "fit_candidates_refined_top16.csv", refined_scores)
    write_candidate_csv(out_dir / "alpha_rgb_projection_candidates_q1.csv", alpha_q1_rgb_projection_scores)
    write_candidate_csv(out_dir / "alpha_rgb_projection_candidates_q2.csv", alpha_q2_rgb_projection_scores)
    write_case_csv(out_dir / "measured_cases.csv", case_rows)
    if winner and write_diffs > 0:
        winner_candidate = next(candidate for candidate in candidates if candidate.id == winner["id"])
        write_winner_diffs(
            out_dir / "winner_diffs",
            q2_cases[:write_diffs],
            images,
            source_pixels,
            winner_candidate,
        )

    report = {
        "schema": "ae-native-renderer.geometry2-sampling2-fit.v1",
        "pack_id": PACK_ID,
        "pack": str(pack),
        "png_root": str(png_root),
        "measured_case_count": len(case_rows),
        "missing_case_count": len(missing),
        "missing_cases": missing[:64],
        "q1_bilinear_sanity": q1_sanity,
        "q2_fit": {
            "case_count": len(q2_cases),
            "winner": winner,
            "current_native_catmull_rom": current,
            "opaque_texture_fit": {
                "q1_bilinear_sanity": opaque_q1_sanity,
                "winner": opaque_scores[0] if opaque_scores else None,
                "current_native_catmull_rom": opaque_current,
                "top_candidates": opaque_scores[:12],
                "note": "Opaque texture cases avoid alpha/premultiply ambiguity and are the primary kernel-family signal.",
            },
            "alpha_steps_fit": {
                "q1_bilinear": {
                    "winner": alpha_q1_scores[0] if alpha_q1_scores else None,
                    "top_candidates": alpha_q1_scores[:12],
                },
                "q2_bicubic": {
                    "winner": alpha_q2_scores[0] if alpha_q2_scores else None,
                    "top_candidates": alpha_q2_scores[:12],
                },
                "rgb_over_black_projection": {
                    "q1_bilinear": {
                        "winner": alpha_q1_rgb_projection_scores[0] if alpha_q1_rgb_projection_scores else None,
                        "top_candidates": alpha_q1_rgb_projection_scores[:12],
                    },
                    "q2_bicubic": {
                        "winner": alpha_q2_rgb_projection_scores[0] if alpha_q2_rgb_projection_scores else None,
                        "top_candidates": alpha_q2_rgb_projection_scores[:12],
                    },
                    "note": "Compares AE RGB-only outputs against candidate pixels projected over black. This is the acceptance metric when the output module did not preserve alpha.",
                },
                "output_observation": summarize_alpha_output(alpha_q1_cases + alpha_q2_cases, images, image_meta),
                "note": "Raw RGBA candidates compare straight/premult variants directly. If output_observation.has_alpha_band_case_count is zero, raw RGBA includes synthetic opaque alpha from RGB-only exports and must not drive internal alpha-policy changes.",
            },
            "top_candidates_refined_full_frame": refined_scores[:16],
            "top_candidates": candidate_scores[:24],
            "method": {
                "pixel_center": "integer",
                "source_uv": "inverse Geometry2 transform matching recovered matrix order for these affine probes",
                "candidate_space": "bilinear baseline, Keys cubic a sweep, Mitchell-Netravali B/C grid, transparent/clamp edges, round/floor output quantization",
                "fit_pass": "sample_step=2 coarse search, then full-frame rescore of top 16 candidates",
            },
        },
    }
    (out_dir / "fit_report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return report


def score_cases(
    rows: Iterable[dict[str, object]],
    images: dict[str, object],
    source_pixels: dict[str, list[tuple[int, int, int, int]]],
    candidate: Candidate,
    sample_step: int,
) -> dict[str, object]:
    totals = 0
    max_delta = 0
    changed_pixels = 0
    total_pixels = 0
    per_case = []
    for row in rows:
        source = source_by_id(str(row["source_id"]))
        transform = TRANSFORM_BY_ID[str(row["transform_id"])]
        image = images[str(row["case_id"])]
        pixels = image.load()
        width, height = image.size
        case_total = 0
        case_max = 0
        case_changed = 0
        for y in range(0, height, sample_step):
            for x in range(0, width, sample_step):
                observed = tuple(int(c) for c in pixels[x, y])
                pred = candidate.sample(
                    source_pixels[source.id],
                    width,
                    height,
                    source_uv(transform, x, y),
                    candidate.edge,
                    candidate.round_mode,
                    **candidate.params,
                )
                pixel_changed = False
                for channel in range(CHANNELS):
                    delta = abs(observed[channel] - pred[channel])
                    totals += delta
                    case_total += delta
                    max_delta = max(max_delta, delta)
                    case_max = max(case_max, delta)
                    pixel_changed = pixel_changed or delta != 0
                if pixel_changed:
                    changed_pixels += 1
                    case_changed += 1
                total_pixels += 1
        per_case.append(
            {
                "case_id": row["case_id"],
                "mean_abs_channel_delta": round(case_total / max(total_pixels_for_step(width, height, sample_step) * CHANNELS, 1), 6),
                "max_channel_delta": case_max,
                "changed_pixel_count": case_changed,
            }
        )
    denom = max(total_pixels * CHANNELS, 1)
    return {
        "id": candidate.id,
        "sampler": candidate.sampler,
        "edge": candidate.edge,
        "round_mode": candidate.round_mode,
        "params": candidate.params,
        "case_count": len(per_case),
        "sample_step": sample_step,
        "mean_abs_channel_delta": round(totals / denom, 6),
        "max_channel_delta": max_delta,
        "changed_pixel_count": changed_pixels,
        "per_case": per_case[:12],
    }


def score_cases_rgb_over_black(
    rows: Iterable[dict[str, object]],
    images: dict[str, object],
    source_pixels: dict[str, list[tuple[int, int, int, int]]],
    candidate: Candidate,
    sample_step: int,
) -> dict[str, object]:
    totals = 0
    max_delta = 0
    changed_pixels = 0
    total_pixels = 0
    per_case = []
    for row in rows:
        source = source_by_id(str(row["source_id"]))
        transform = TRANSFORM_BY_ID[str(row["transform_id"])]
        image = images[str(row["case_id"])]
        pixels = image.load()
        width, height = image.size
        case_total = 0
        case_max = 0
        case_changed = 0
        for y in range(0, height, sample_step):
            for x in range(0, width, sample_step):
                observed = tuple(int(c) for c in pixels[x, y][:3])
                rgba = candidate.sample(
                    source_pixels[source.id],
                    width,
                    height,
                    source_uv(transform, x, y),
                    candidate.edge,
                    candidate.round_mode,
                    **candidate.params,
                )
                pred = project_rgb_over_black(rgba, candidate)
                pixel_changed = False
                for channel in range(3):
                    delta = abs(observed[channel] - pred[channel])
                    totals += delta
                    case_total += delta
                    max_delta = max(max_delta, delta)
                    case_max = max(case_max, delta)
                    pixel_changed = pixel_changed or delta != 0
                if pixel_changed:
                    changed_pixels += 1
                    case_changed += 1
                total_pixels += 1
        per_case.append(
            {
                "case_id": row["case_id"],
                "mean_abs_channel_delta": round(case_total / max(total_pixels_for_step(width, height, sample_step) * 3, 1), 6),
                "max_channel_delta": case_max,
                "changed_pixel_count": case_changed,
            }
        )
    denom = max(total_pixels * 3, 1)
    return {
        "id": candidate.id,
        "sampler": candidate.sampler,
        "edge": candidate.edge,
        "round_mode": candidate.round_mode,
        "params": candidate.params,
        "case_count": len(per_case),
        "sample_step": sample_step,
        "metric_policy": "rgb_over_black_projection",
        "mean_abs_channel_delta": round(totals / denom, 6),
        "max_channel_delta": max_delta,
        "changed_pixel_count": changed_pixels,
        "per_case": per_case[:12],
    }


def project_rgb_over_black(rgba: tuple[int, int, int, int], candidate: Candidate) -> tuple[int, int, int]:
    if "premult_keep" in candidate.sampler:
        return rgba[:3]
    alpha = rgba[3] / 255.0
    return tuple(quantize(rgba[channel] * alpha, candidate.round_mode) for channel in range(3))


def summarize_alpha_output(
    rows: list[dict[str, object]],
    images: dict[str, object],
    image_meta: dict[str, dict[str, object]],
) -> dict[str, object]:
    modes: dict[str, int] = {}
    alpha_band_cases = 0
    alpha_extrema = []
    for row in rows:
        cid = str(row["case_id"])
        meta = image_meta.get(cid, {})
        mode = str(meta.get("mode") or "unknown")
        bands = ",".join(str(part) for part in meta.get("bands", []))
        modes[f"{mode}:{bands}"] = modes.get(f"{mode}:{bands}", 0) + 1
        if bool(meta.get("has_alpha_band")):
            alpha_band_cases += 1
        image = images[cid]
        extrema = image.getchannel("A").getextrema()
        alpha_extrema.append({"case_id": cid, "min": int(extrema[0]), "max": int(extrema[1])})
    return {
        "case_count": len(rows),
        "mode_counts": modes,
        "has_alpha_band_case_count": alpha_band_cases,
        "all_cases_have_alpha_band": alpha_band_cases == len(rows) if rows else False,
        "converted_rgba_alpha_extrema_sample": alpha_extrema[:12],
    }


def total_pixels_for_step(width: int, height: int, step: int) -> int:
    return len(range(0, width, step)) * len(range(0, height, step))


def build_candidates() -> list[Candidate]:
    out = [
        Candidate("bilinear_transparent_round", "bilinear", "transparent", "round", {}, bilinear_sample),
        Candidate("bilinear_clamp_round", "bilinear", "clamp", "round", {}, bilinear_sample),
    ]
    for edge in ["transparent", "clamp"]:
        for round_mode in ["round", "floor"]:
            for i in range(-12, 5):
                a = i / 10.0
                out.append(
                    Candidate(
                        f"keys_a_{a:+.3f}_{edge}_{round_mode}".replace("+", ""),
                        "keys_cubic",
                        edge,
                        round_mode,
                        {"a": a},
                        keys_cubic_sample,
                    )
                )
            for b in [0.0, 1.0 / 3.0, 0.5, 1.0]:
                for c in [0.0, 1.0 / 3.0, 0.5, 1.0]:
                    out.append(
                        Candidate(
                            f"mitchell_b_{b:.3f}_c_{c:.3f}_{edge}_{round_mode}",
                            "mitchell_netravali",
                            edge,
                            round_mode,
                            {"b": b, "c": c},
                            mitchell_sample,
                        )
                    )
    return out


def build_alpha_candidates(sampler: str) -> list[Candidate]:
    if sampler == "bilinear":
        return [
            Candidate("bilinear_straight_transparent_round", "bilinear_straight", "transparent", "round", {}, bilinear_sample),
            Candidate(
                "bilinear_premult_unpremultiply_transparent_round",
                "bilinear_premult_unpremultiply",
                "transparent",
                "round",
                {},
                bilinear_sample_premult_unpremultiply,
            ),
            Candidate(
                "bilinear_premult_keep_transparent_round",
                "bilinear_premult_keep",
                "transparent",
                "round",
                {},
                bilinear_sample_premult_keep,
            ),
            Candidate("bilinear_straight_clamp_round", "bilinear_straight", "clamp", "round", {}, bilinear_sample),
            Candidate(
                "bilinear_premult_unpremultiply_clamp_round",
                "bilinear_premult_unpremultiply",
                "clamp",
                "round",
                {},
                bilinear_sample_premult_unpremultiply,
            ),
            Candidate(
                "bilinear_premult_keep_clamp_round",
                "bilinear_premult_keep",
                "clamp",
                "round",
                {},
                bilinear_sample_premult_keep,
            ),
        ]
    if sampler == "bicubic":
        return [
            Candidate(
                "keys_a_-0.700_straight_transparent_round",
                "keys_cubic_straight",
                "transparent",
                "round",
                {"a": -0.7},
                keys_cubic_sample,
            ),
            Candidate(
                "keys_a_-0.700_premult_unpremultiply_transparent_round",
                "keys_cubic_premult_unpremultiply",
                "transparent",
                "round",
                {"a": -0.7},
                keys_cubic_sample_premult_unpremultiply,
            ),
            Candidate(
                "keys_a_-0.700_premult_keep_transparent_round",
                "keys_cubic_premult_keep",
                "transparent",
                "round",
                {"a": -0.7},
                keys_cubic_sample_premult_keep,
            ),
            Candidate(
                "keys_a_-0.700_straight_clamp_round",
                "keys_cubic_straight",
                "clamp",
                "round",
                {"a": -0.7},
                keys_cubic_sample,
            ),
            Candidate(
                "keys_a_-0.700_premult_unpremultiply_clamp_round",
                "keys_cubic_premult_unpremultiply",
                "clamp",
                "round",
                {"a": -0.7},
                keys_cubic_sample_premult_unpremultiply,
            ),
            Candidate(
                "keys_a_-0.700_premult_keep_clamp_round",
                "keys_cubic_premult_keep",
                "clamp",
                "round",
                {"a": -0.7},
                keys_cubic_sample_premult_keep,
            ),
        ]
    raise ValueError(sampler)


def source_by_id(source_id: str) -> SourceDef:
    for source in SOURCES:
        if source.id == source_id:
            return source
    raise KeyError(source_id)


def source_uv(transform: TransformDef, x: int, y: int) -> tuple[float, float]:
    ax, ay = transform.anchor
    px, py = transform.position
    sx = transform.scale[0] / 100.0 if abs(transform.scale[0]) > 1e-9 else 1.0
    sy = transform.scale[1] / 100.0 if abs(transform.scale[1]) > 1e-9 else 1.0
    radians = math.radians(transform.rotation)
    cos_v = math.cos(radians)
    sin_v = math.sin(radians)
    dx = float(x) - px
    dy = float(y) - py
    unrot_x = cos_v * dx + sin_v * dy
    unrot_y = -sin_v * dx + cos_v * dy
    return (ax + unrot_x / sx, ay + unrot_y / sy)


def bilinear_sample(
    pixels: list[tuple[int, int, int, int]],
    width: int,
    height: int,
    uv: tuple[float, float],
    edge: str,
    round_mode: str,
    **_params: float,
) -> tuple[int, int, int, int]:
    x, y = uv
    x0 = math.floor(x)
    y0 = math.floor(y)
    tx = x - x0
    ty = y - y0
    weights_x = [(x0, 1.0 - tx), (x0 + 1, tx)]
    weights_y = [(y0, 1.0 - ty), (y0 + 1, ty)]
    return weighted_sample(pixels, width, height, weights_x, weights_y, edge, round_mode)


def bilinear_sample_premult_unpremultiply(
    pixels: list[tuple[int, int, int, int]],
    width: int,
    height: int,
    uv: tuple[float, float],
    edge: str,
    round_mode: str,
    **_params: float,
) -> tuple[int, int, int, int]:
    x, y = uv
    x0 = math.floor(x)
    y0 = math.floor(y)
    tx = x - x0
    ty = y - y0
    weights_x = [(x0, 1.0 - tx), (x0 + 1, tx)]
    weights_y = [(y0, 1.0 - ty), (y0 + 1, ty)]
    return weighted_sample_premult(pixels, width, height, weights_x, weights_y, edge, round_mode, "unpremultiply")


def bilinear_sample_premult_keep(
    pixels: list[tuple[int, int, int, int]],
    width: int,
    height: int,
    uv: tuple[float, float],
    edge: str,
    round_mode: str,
    **_params: float,
) -> tuple[int, int, int, int]:
    x, y = uv
    x0 = math.floor(x)
    y0 = math.floor(y)
    tx = x - x0
    ty = y - y0
    weights_x = [(x0, 1.0 - tx), (x0 + 1, tx)]
    weights_y = [(y0, 1.0 - ty), (y0 + 1, ty)]
    return weighted_sample_premult(pixels, width, height, weights_x, weights_y, edge, round_mode, "keep")


def keys_cubic_sample(
    pixels: list[tuple[int, int, int, int]],
    width: int,
    height: int,
    uv: tuple[float, float],
    edge: str,
    round_mode: str,
    *,
    a: float,
) -> tuple[int, int, int, int]:
    x, y = uv
    x0 = math.floor(x)
    y0 = math.floor(y)
    tx = x - x0
    ty = y - y0
    wx = keys_weights(tx, a)
    wy = keys_weights(ty, a)
    return weighted_sample(
        pixels,
        width,
        height,
        [(x0 + i - 1, wx[i]) for i in range(4)],
        [(y0 + i - 1, wy[i]) for i in range(4)],
        edge,
        round_mode,
    )


def keys_cubic_sample_premult_unpremultiply(
    pixels: list[tuple[int, int, int, int]],
    width: int,
    height: int,
    uv: tuple[float, float],
    edge: str,
    round_mode: str,
    *,
    a: float,
) -> tuple[int, int, int, int]:
    x, y = uv
    x0 = math.floor(x)
    y0 = math.floor(y)
    tx = x - x0
    ty = y - y0
    wx = keys_weights(tx, a)
    wy = keys_weights(ty, a)
    return weighted_sample_premult(
        pixels,
        width,
        height,
        [(x0 + i - 1, wx[i]) for i in range(4)],
        [(y0 + i - 1, wy[i]) for i in range(4)],
        edge,
        round_mode,
        "unpremultiply",
    )


def keys_cubic_sample_premult_keep(
    pixels: list[tuple[int, int, int, int]],
    width: int,
    height: int,
    uv: tuple[float, float],
    edge: str,
    round_mode: str,
    *,
    a: float,
) -> tuple[int, int, int, int]:
    x, y = uv
    x0 = math.floor(x)
    y0 = math.floor(y)
    tx = x - x0
    ty = y - y0
    wx = keys_weights(tx, a)
    wy = keys_weights(ty, a)
    return weighted_sample_premult(
        pixels,
        width,
        height,
        [(x0 + i - 1, wx[i]) for i in range(4)],
        [(y0 + i - 1, wy[i]) for i in range(4)],
        edge,
        round_mode,
        "keep",
    )


def mitchell_sample(
    pixels: list[tuple[int, int, int, int]],
    width: int,
    height: int,
    uv: tuple[float, float],
    edge: str,
    round_mode: str,
    *,
    b: float,
    c: float,
) -> tuple[int, int, int, int]:
    x, y = uv
    x0 = math.floor(x)
    y0 = math.floor(y)
    wx = [mitchell_weight(abs((i - 1) - (x - x0)), b, c) for i in range(4)]
    wy = [mitchell_weight(abs((i - 1) - (y - y0)), b, c) for i in range(4)]
    return weighted_sample(
        pixels,
        width,
        height,
        [(x0 + i - 1, wx[i]) for i in range(4)],
        [(y0 + i - 1, wy[i]) for i in range(4)],
        edge,
        round_mode,
    )


def keys_weights(t: float, a: float) -> list[float]:
    return [keys_weight(1.0 + t, a), keys_weight(t, a), keys_weight(1.0 - t, a), keys_weight(2.0 - t, a)]


def keys_weight(x: float, a: float) -> float:
    x = abs(x)
    if x < 1.0:
        return (a + 2.0) * x * x * x - (a + 3.0) * x * x + 1.0
    if x < 2.0:
        return a * x * x * x - 5.0 * a * x * x + 8.0 * a * x - 4.0 * a
    return 0.0


def mitchell_weight(x: float, b: float, c: float) -> float:
    if x < 1.0:
        return ((12 - 9 * b - 6 * c) * x**3 + (-18 + 12 * b + 6 * c) * x**2 + (6 - 2 * b)) / 6.0
    if x < 2.0:
        return ((-b - 6 * c) * x**3 + (6 * b + 30 * c) * x**2 + (-12 * b - 48 * c) * x + (8 * b + 24 * c)) / 6.0
    return 0.0


def weighted_sample(
    pixels: list[tuple[int, int, int, int]],
    width: int,
    height: int,
    weights_x: list[tuple[int, float]],
    weights_y: list[tuple[int, float]],
    edge: str,
    round_mode: str,
) -> tuple[int, int, int, int]:
    accum = [0.0, 0.0, 0.0, 0.0]
    for sy, wy in weights_y:
        for sx, wx in weights_x:
            weight = wx * wy
            if edge == "clamp":
                ix = min(width - 1, max(0, sx))
                iy = min(height - 1, max(0, sy))
                sample = pixels[iy * width + ix]
            elif edge == "transparent":
                if sx < 0 or sy < 0 or sx >= width or sy >= height:
                    sample = (0, 0, 0, 0)
                else:
                    sample = pixels[sy * width + sx]
            else:
                raise ValueError(edge)
            for channel in range(CHANNELS):
                accum[channel] += sample[channel] * weight
    return tuple(quantize(value, round_mode) for value in accum)


def weighted_sample_premult(
    pixels: list[tuple[int, int, int, int]],
    width: int,
    height: int,
    weights_x: list[tuple[int, float]],
    weights_y: list[tuple[int, float]],
    edge: str,
    round_mode: str,
    output_policy: str,
) -> tuple[int, int, int, int]:
    premult = [0.0, 0.0, 0.0]
    alpha = 0.0
    for sy, wy in weights_y:
        for sx, wx in weights_x:
            weight = wx * wy
            if edge == "clamp":
                ix = min(width - 1, max(0, sx))
                iy = min(height - 1, max(0, sy))
                sample = pixels[iy * width + ix]
            elif edge == "transparent":
                if sx < 0 or sy < 0 or sx >= width or sy >= height:
                    sample = (0, 0, 0, 0)
                else:
                    sample = pixels[sy * width + sx]
            else:
                raise ValueError(edge)
            sample_alpha = sample[3] / 255.0
            for channel in range(3):
                premult[channel] += sample[channel] * sample_alpha * weight
            alpha += sample[3] * weight
    if output_policy == "keep":
        rgb = premult
    elif output_policy == "unpremultiply":
        alpha_norm = alpha / 255.0
        rgb = [0.0, 0.0, 0.0] if alpha_norm <= 1e-9 else [value / alpha_norm for value in premult]
    else:
        raise ValueError(output_policy)
    return (
        quantize(rgb[0], round_mode),
        quantize(rgb[1], round_mode),
        quantize(rgb[2], round_mode),
        quantize(alpha, round_mode),
    )


def quantize(value: float, mode: str) -> int:
    value = max(0.0, min(255.0, value))
    if mode == "floor":
        return int(math.floor(value))
    if mode == "round":
        return int(math.floor(value + 0.5))
    raise ValueError(mode)


def write_winner_diffs(
    out_dir: Path,
    rows: list[dict[str, object]],
    images: dict[str, object],
    source_pixels: dict[str, list[tuple[int, int, int, int]]],
    candidate: Candidate,
) -> None:
    from PIL import Image

    out_dir.mkdir(parents=True, exist_ok=True)
    for row in rows:
        source = source_by_id(str(row["source_id"]))
        transform = TRANSFORM_BY_ID[str(row["transform_id"])]
        image = images[str(row["case_id"])]
        observed_pixels = image.load()
        width, height = image.size
        pred = Image.new("RGBA", (width, height))
        diff = Image.new("RGBA", (width, height))
        pred_pixels = pred.load()
        diff_pixels = diff.load()
        for y in range(height):
            for x in range(width):
                rgba = candidate.sample(
                    source_pixels[source.id],
                    width,
                    height,
                    source_uv(transform, x, y),
                    candidate.edge,
                    candidate.round_mode,
                    **candidate.params,
                )
                pred_pixels[x, y] = rgba
                obs = observed_pixels[x, y]
                diff_pixels[x, y] = tuple(abs(int(obs[i]) - int(rgba[i])) for i in range(4))
        stem = str(row["case_id"])
        pred.save(out_dir / f"{stem}__native_{candidate.id}.png")
        diff.save(out_dir / f"{stem}__diff_{candidate.id}.png")


def write_candidate_csv(path: Path, rows: list[dict[str, object]]) -> None:
    fields = ["id", "sampler", "edge", "round_mode", "params", "case_count", "mean_abs_channel_delta", "max_channel_delta", "changed_pixel_count"]
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        for row in rows:
            writer.writerow({field: json.dumps(row[field]) if field == "params" else row[field] for field in fields})


def write_case_csv(path: Path, rows: list[dict[str, object]]) -> None:
    fields = ["case_id", "source_id", "transform_id", "sampling_id", "sampling_value", "png"]
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
    if source_id == "fit_texture":
        return [fit_texture_pixel(x, y) for y in range(SIZE) for x in range(SIZE)]
    if source_id == "impulse_grid":
        return [impulse_grid_pixel(x, y) for y in range(SIZE) for x in range(SIZE)]
    if source_id == "alpha_steps":
        return [alpha_steps_pixel(x, y) for y in range(SIZE) for x in range(SIZE)]
    raise ValueError(source_id)


def fit_texture_pixel(x: int, y: int) -> tuple[int, int, int, int]:
    return (
        (37 * x + 17 * y + 3 * x * y) % 256,
        (11 * x + 53 * y + 5 * x * y + 91) % 256,
        (97 * x + 29 * y + 7 * x * y + 13) % 256,
        255,
    )


def impulse_grid_pixel(x: int, y: int) -> tuple[int, int, int, int]:
    edge = x in {0, SIZE - 1} or y in {0, SIZE - 1}
    grid = x % 8 == 0 and y % 8 == 0
    if not edge and not grid:
        return (0, 0, 0, 0)
    return ((31 * x + 17) % 256, (47 * y + 19) % 256, (13 * (x + y) + 101) % 256, 255)


def alpha_steps_pixel(x: int, y: int) -> tuple[int, int, int, int]:
    alpha = [0, 64, 128, 192, 255][((x // 8) + 2 * (y // 8)) % 5]
    return ((19 * x + 41) % 256, (23 * y + 67) % 256, (7 * x + 11 * y + 89) % 256, alpha)


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
    with tempfile.TemporaryDirectory(prefix="geometry2_sampling2_fit_selftest_") as tmp:
        pack = Path(tmp)
        generate_assets(pack)
        png_root = pack / "ae_goldens" / "png8"
        render_synthetic_pack(png_root)
        report = fit_pack(pack, png_root, pack / "ae_goldens" / "fit_reports", write_diffs=1)
        winner = report["q2_fit"]["winner"]
        if not str(winner["id"]).startswith("keys_a_-0.500_transparent_round"):
            raise SystemExit(f"self-test failed: expected catmull-rom winner, got {winner['id']}")
        print("self-test ok")
    return 0


def render_synthetic_pack(png_root: Path) -> None:
    source_pixels = {source.id: primitive_pixels(source.id) for source in SOURCES}
    candidate = Candidate("keys_a_-0.500_transparent_round", "keys_cubic", "transparent", "round", {"a": -0.5}, keys_cubic_sample)
    for expected in expected_cases():
        cid = str(expected["id"])
        source = expected["source"]
        transform = expected["transform"]
        sampling = expected["sampling"]
        assert isinstance(source, SourceDef)
        assert isinstance(transform, TransformDef)
        assert isinstance(sampling, SamplingDef)
        sample = bilinear_sample if sampling.value == 1 else candidate.sample
        params = {} if sampling.value == 1 else candidate.params
        pixels = []
        for y in range(SIZE):
            for x in range(SIZE):
                pixels.append(
                    sample(source_pixels[source.id], SIZE, SIZE, source_uv(transform, x, y), "transparent", "round", **params)
                )
        save_png(png_root / cid / f"{cid}_00000.png", SIZE, SIZE, pixels)


if __name__ == "__main__":
    raise SystemExit(main())
