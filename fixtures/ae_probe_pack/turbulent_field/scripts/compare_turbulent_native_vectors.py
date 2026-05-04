#!/usr/bin/env python3
"""Compare AE Turbulent Displace vector measurements to the native field model.

This is a fitting/reporting harness, not a formula tuning patch. The native
model below mirrors the current Rust implementation in
`crates/effects/src/turbulent_displace.rs` so Round 5 AE measurements can be
compared sample-by-sample without changing the effects crate.

TODO: replace this Python mirror with a small native telemetry CLI/API that can
emit arbitrary sample points from the Rust field model. Keeping the mirror here
is deliberately scoped to measurement plumbing.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import re
from dataclasses import dataclass, asdict
from pathlib import Path
from statistics import mean
from typing import Any

import measure_turbulent_vectors as ae_measure


WIDTH = 512
HEIGHT = 512
FPS = 30.0
ANIM_DURATION_SECONDS = 2.0
REPRESENTATIVE_CASES = [
    "TD_AMOUNT_SWEEP_A000",
    "TD_AMOUNT_SWEEP_A001",
    "TD_AMOUNT_SWEEP_A010",
    "TD_AMOUNT_SWEEP_A045",
    "TD_AMOUNT_SWEEP_A100",
    "TD_SIZE_SWEEP_S065",
    "TD_DISPLACEMENT_TYPE_01",
    "TD_DISPLACEMENT_TYPE_09",
    "TD_SEED_SWEEP_999",
    "TD_RESIZE_LAYER_ON",
]

SIGNED_PERMUTATION_CANDIDATES = [
    ("identity", [[1.0, 0.0], [0.0, 1.0]]),
    ("flip_x", [[-1.0, 0.0], [0.0, 1.0]]),
    ("flip_y", [[1.0, 0.0], [0.0, -1.0]]),
    ("flip_xy", [[-1.0, 0.0], [0.0, -1.0]]),
    ("swap_xy", [[0.0, 1.0], [1.0, 0.0]]),
    ("rot90_ccw", [[0.0, -1.0], [1.0, 0.0]]),
    ("rot90_cw", [[0.0, 1.0], [-1.0, 0.0]]),
    ("swap_xy_flip_xy", [[0.0, -1.0], [-1.0, 0.0]]),
]


@dataclass(frozen=True)
class FieldParams:
    displacement_type: float = 1.0
    amount: float = 45.0
    size: float = 65.0
    offset: tuple[float, float] = (256.0, 256.0)
    complexity: float = 2.0
    evolution: float = 0.0
    random_seed: float = 0.0
    pinning: float = 3.0
    resize_layer: bool = False


@dataclass(frozen=True)
class ResolvedParams:
    displacement_type: int
    amount: float
    size: float
    offset: tuple[float, float]
    complexity: int
    evolution: float
    random_seed: int
    pinning: int
    resize_layer: bool
    amplitude: float
    phase_radians: float


@dataclass(frozen=True)
class NativeSample:
    output_xy: tuple[int, int]
    noise: tuple[float, float]
    displacement: tuple[float, float]
    source_uv: tuple[float, float]
    sample_xy: tuple[int, int] | None
    out_of_bounds: bool

    @property
    def sampled_displacement(self) -> tuple[float, float] | None:
        if self.sample_xy is None:
            return None
        x, y = self.output_xy
        sx, sy = self.sample_xy
        return float(sx - x), float(sy - y)


def clamp(value: float, lo: float, hi: float) -> float:
    return min(hi, max(lo, value))


def round_away_from_zero(value: float) -> int:
    if value >= 0.0:
        return math.floor(value + 0.5)
    return math.ceil(value - 0.5)


def percentile(values: list[float], p: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    idx = min(len(ordered) - 1, max(0, round((len(ordered) - 1) * p)))
    return ordered[idx]


def transform_vector(matrix: list[list[float]], dx: float, dy: float) -> tuple[float, float]:
    return (
        matrix[0][0] * dx + matrix[0][1] * dy,
        matrix[1][0] * dx + matrix[1][1] * dy,
    )


def scan_signed_permutation_basis(rows: list[dict[str, float]]) -> dict[str, Any]:
    if not rows:
        return {"sample_count": 0, "candidates": []}

    candidates = []
    for name, matrix in SIGNED_PERMUTATION_CANDIDATES:
        vector_errors = []
        abs_dx_errors = []
        abs_dy_errors = []
        signed_dx_errors = []
        signed_dy_errors = []
        cosines = []
        opposite_count = 0
        for row in rows:
            tx, ty = transform_vector(matrix, row["native_dx"], row["native_dy"])
            dx_error = tx - row["ae_dx"]
            dy_error = ty - row["ae_dy"]
            vector_errors.append(math.hypot(dx_error, dy_error))
            abs_dx_errors.append(abs(dx_error))
            abs_dy_errors.append(abs(dy_error))
            signed_dx_errors.append(dx_error)
            signed_dy_errors.append(dy_error)

            ae_mag = row["ae_magnitude"]
            native_mag = math.hypot(tx, ty)
            if ae_mag > 1.1920929e-7 and native_mag > 1.1920929e-7:
                dot = tx * row["ae_dx"] + ty * row["ae_dy"]
                if dot < 0.0:
                    opposite_count += 1
                cosines.append(clamp(dot / (ae_mag * native_mag), -1.0, 1.0))

        candidates.append(
            {
                "candidate": name,
                "matrix": matrix,
                "sample_count": len(rows),
                "mean_abs_dx_error": mean(abs_dx_errors),
                "mean_abs_dy_error": mean(abs_dy_errors),
                "mean_signed_dx_error": mean(signed_dx_errors),
                "mean_signed_dy_error": mean(signed_dy_errors),
                "mean_vector_error": mean(vector_errors),
                "p95_vector_error": percentile(vector_errors, 0.95),
                "max_vector_error": max(vector_errors),
                "mean_cosine": mean(cosines) if cosines else 0.0,
                "opposite_direction_fraction": (
                    float(opposite_count) / float(len(cosines)) if cosines else 0.0
                ),
            }
        )

    sorted_candidates = sorted(candidates, key=lambda item: item["mean_vector_error"])
    identity = next(item for item in candidates if item["candidate"] == "identity")
    best = sorted_candidates[0]
    improvement = identity["mean_vector_error"] - best["mean_vector_error"]
    return {
        "sample_count": len(rows),
        "identity": identity,
        "best": best,
        "best_over_identity_mean_vector_improvement": improvement,
        "best_over_identity_mean_vector_improvement_fraction": (
            improvement / identity["mean_vector_error"]
            if identity["mean_vector_error"] > 1.1920929e-7
            else 0.0
        ),
        "candidates": sorted_candidates,
    }


def resolve_params(params: FieldParams) -> ResolvedParams:
    amount = clamp(params.amount, 0.0, 200.0)
    size = clamp(params.size, 2.0, 1000.0)
    complexity = int(clamp(round_away_from_zero(params.complexity), 1.0, 6.0))
    random_seed = int(clamp(round_away_from_zero(params.random_seed), 0.0, 2**32 - 1))
    phase_radians = math.radians(params.evolution) + seed_phase(random_seed)
    return ResolvedParams(
        displacement_type=int(clamp(round_away_from_zero(params.displacement_type), 1.0, 9.0)),
        amount=amount,
        size=size,
        offset=params.offset,
        complexity=complexity,
        evolution=params.evolution,
        random_seed=random_seed,
        pinning=int(clamp(round_away_from_zero(params.pinning), 0.0, 17.0)),
        resize_layer=params.resize_layer,
        amplitude=amount * 0.25,
        phase_radians=phase_radians,
    )


def seed_phase(seed: int) -> float:
    return float(seed) * 0.618_034


def turbulence(x: float, y: float, phase: float, octaves: int) -> float:
    value = 0.0
    amplitude = 1.0
    frequency = 1.0
    total = 0.0
    for octave in range(octaves):
        angle = x * 12.9898 * frequency + y * 78.233 * frequency + phase + octave * 4.123
        value += math.sin(angle) * amplitude
        total += amplitude
        amplitude *= 0.5
        frequency *= 2.0
    return clamp(value / total, -1.0, 1.0)


def field_noise(resolved: ResolvedParams, x: int, y: int) -> tuple[float, float]:
    seed = float(resolved.random_seed)
    nx = (float(x) - resolved.offset[0]) / resolved.size
    ny = (float(y) - resolved.offset[1]) / resolved.size
    return (
        turbulence(
            nx + 17.0 + seed * 0.137,
            ny + seed * 0.071,
            resolved.phase_radians,
            resolved.complexity,
        ),
        turbulence(
            nx + seed * 0.113,
            ny + 29.0 + seed * 0.193,
            resolved.phase_radians + 1.7,
            resolved.complexity,
        ),
    )


def radial_basis(resolved: ResolvedParams, x: int, y: int) -> tuple[float, float]:
    dx = float(x) - resolved.offset[0]
    dy = float(y) - resolved.offset[1]
    length = math.hypot(dx, dy)
    if length <= 1.1920929e-7:
        return 1.0, 0.0
    return dx / length, dy / length


def displacement_from_noise(
    resolved: ResolvedParams,
    noise: tuple[float, float],
    x: int,
    y: int,
) -> tuple[float, float]:
    if resolved.amplitude <= 1.1920929e-7:
        return 0.0, 0.0
    base = (noise[0] * resolved.amplitude, noise[1] * resolved.amplitude)
    scalar = ((noise[0] + noise[1]) * 0.5) * resolved.amplitude
    radial = radial_basis(resolved, x, y)
    tangent = (-radial[1], radial[0])

    if resolved.displacement_type == 2:
        return radial[0] * scalar * 1.5, radial[1] * scalar * 1.5
    if resolved.displacement_type in (3, 4):
        return tangent[0] * scalar * 1.5, tangent[1] * scalar * 1.5
    if resolved.displacement_type == 5:
        return base[0] * 1.45, base[1] * 0.35
    if resolved.displacement_type == 6:
        return base[0] * 0.35, base[1] * 0.85
    if resolved.displacement_type in (7, 8):
        return base[1] * 0.85, base[0] * 0.85
    if resolved.displacement_type == 9:
        return 0.0, scalar
    return base


def uses_clamped_edges(resolved: ResolvedParams) -> bool:
    return resolved.pinning > 0 or resolved.resize_layer


def sample_nearest_round(
    source_uv: tuple[float, float],
    resolved: ResolvedParams,
    width: int = WIDTH,
    height: int = HEIGHT,
) -> tuple[tuple[int, int] | None, bool]:
    sx = float(round_away_from_zero(source_uv[0]))
    sy = float(round_away_from_zero(source_uv[1]))
    if sx < 0.0 or sy < 0.0 or sx >= float(width) or sy >= float(height):
        if uses_clamped_edges(resolved) and width > 0 and height > 0:
            return (
                (
                    int(clamp(sx, 0.0, float(width - 1))),
                    int(clamp(sy, 0.0, float(height - 1))),
                ),
                True,
            )
        return None, True
    return (int(sx), int(sy)), False


def native_sample(params: FieldParams, x: int, y: int) -> NativeSample:
    resolved = resolve_params(params)
    noise = field_noise(resolved, x, y)
    displacement = displacement_from_noise(resolved, noise, x, y)
    source_uv = (float(x) + displacement[0], float(y) + displacement[1])
    sample_xy, out_of_bounds = sample_nearest_round(source_uv, resolved)
    return NativeSample(
        output_xy=(x, y),
        noise=noise,
        displacement=displacement,
        source_uv=source_uv,
        sample_xy=sample_xy,
        out_of_bounds=out_of_bounds,
    )


def case_params(case_id: str, frame: int) -> tuple[FieldParams | None, list[str]]:
    params = FieldParams()
    notes: list[str] = []

    def replace(**kwargs: Any) -> FieldParams:
        fields = asdict(params)
        fields.update(kwargs)
        return FieldParams(**fields)

    if match := re.fullmatch(r"TD_AMOUNT_SWEEP_A(\d+)", case_id):
        return replace(amount=float(int(match.group(1)))), notes
    if match := re.fullmatch(r"TD_SIZE_SWEEP_S(\d+)", case_id):
        size = float(int(match.group(1)))
        if int(size) == 1:
            size = 100.0
            notes.append("AE rejected size=1 in the Round 5 builder; comparison uses AE default size=100.")
        return replace(size=size), notes
    if match := re.fullmatch(r"TD_COMPLEXITY_SWEEP_C(\d+)", case_id):
        return replace(complexity=float(int(match.group(1)))), notes
    if match := re.fullmatch(r"TD_EVOLUTION_STATIC_E(\d+)", case_id):
        return replace(evolution=float(int(match.group(1)))), notes
    if case_id == "TD_EVOLUTION_ANIM":
        evolution = (float(frame) / FPS) / ANIM_DURATION_SECONDS * 180.0
        notes.append("Animated evolution mapped from 0..180 degrees over 2 seconds at 30 fps.")
        return replace(evolution=evolution), notes
    if match := re.fullmatch(r"TD_DISPLACEMENT_TYPE_(\d+)", case_id):
        return replace(displacement_type=float(int(match.group(1)))), notes
    if match := re.fullmatch(r"TD_SEED_SWEEP_(\d+)", case_id):
        return replace(random_seed=float(int(match.group(1)))), notes
    if match := re.fullmatch(r"TD_RESIZE_LAYER_(ON|OFF)", case_id):
        return replace(amount=120.0, size=48.0, resize_layer=match.group(1) == "ON"), notes
    if match := re.fullmatch(r"TD_SAMPLER_CHECK_COORD_A(\d+)", case_id):
        return replace(amount=float(int(match.group(1))), size=256.0, complexity=1.0), notes

    return None, [f"Case is not mapped by this comparison harness: {case_id}"]


def load_samples_for_frame(
    frame: dict[str, Any],
    measurements_path: Path,
    measurements: dict[str, Any],
) -> list[dict[str, Any]]:
    samples = frame.get("samples")
    if samples is not None:
        return samples

    path = resolve_frame_path(str(frame["path"]), measurements_path, measurements)
    _report, samples = ae_measure.measure_frame(path, include_samples=True)
    return samples


def resolve_frame_path(path_text: str, measurements_path: Path, measurements: dict[str, Any]) -> Path:
    path = Path(path_text)
    candidates = []
    if path.is_absolute():
        candidates.append(path)
    else:
        candidates.append(Path.cwd() / path)
        candidates.append(measurements_path.parent / path)

    pack_text = measurements.get("pack")
    if pack_text:
        pack = Path(pack_text)
        if not pack.is_absolute():
            pack = Path.cwd() / pack
        candidates.append(pack / path)
        parts = path.parts
        if "ae_goldens" in parts:
            idx = parts.index("ae_goldens")
            candidates.append(pack / Path(*parts[idx:]))

    # Measurements usually live at <pack>/ae_goldens/metadata/report.json.
    inferred_pack = measurements_path.parent.parent.parent
    parts = path.parts
    if "ae_goldens" in parts:
        idx = parts.index("ae_goldens")
        candidates.append(inferred_pack / Path(*parts[idx:]))

    for candidate in candidates:
        if candidate.exists():
            return candidate
    tried = "\n  ".join(str(candidate) for candidate in candidates)
    raise FileNotFoundError(f"Could not locate frame PNG for {path_text}. Tried:\n  {tried}")


def compare_frame(
    case_id: str,
    frame: dict[str, Any],
    samples: list[dict[str, Any]],
    include_parity_mismatches: bool,
) -> dict[str, Any]:
    frame_number = int(frame["frame"])
    params, notes = case_params(case_id, frame_number)
    if params is None:
        return {
            "case_id": case_id,
            "frame": frame_number,
            "status": "skipped",
            "notes": notes,
        }

    rows: list[dict[str, float]] = []
    skipped_transparent = 0
    skipped_parity_mismatch = 0
    skipped_native_oob = 0
    center: dict[str, Any] | None = None

    for sample in samples:
        if not sample.get("decoded"):
            skipped_transparent += 1
            continue
        if not include_parity_mismatches and not sample.get("parity_match", True):
            skipped_parity_mismatch += 1
            if sample.get("output_x") != WIDTH // 2 or sample.get("output_y") != HEIGHT // 2:
                continue

        x = int(sample["output_x"])
        y = int(sample["output_y"])
        native = native_sample(params, x, y)
        native_dxdy = native.sampled_displacement
        if native_dxdy is None:
            skipped_native_oob += 1
            continue

        ae_dx = float(sample["dx"])
        ae_dy = float(sample["dy"])
        native_dx, native_dy = native_dxdy
        dx_error = native_dx - ae_dx
        dy_error = native_dy - ae_dy
        ae_magnitude = math.hypot(ae_dx, ae_dy)
        native_magnitude = math.hypot(native_dx, native_dy)
        row = {
            "output_x": float(x),
            "output_y": float(y),
            "ae_dx": ae_dx,
            "ae_dy": ae_dy,
            "native_dx": native_dx,
            "native_dy": native_dy,
            "dx_error": dx_error,
            "dy_error": dy_error,
            "abs_dx_error": abs(dx_error),
            "abs_dy_error": abs(dy_error),
            "vector_error": math.hypot(dx_error, dy_error),
            "ae_magnitude": ae_magnitude,
            "native_magnitude": native_magnitude,
            "magnitude_error": native_magnitude - ae_magnitude,
            "abs_magnitude_error": abs(native_magnitude - ae_magnitude),
        }

        is_center = x == WIDTH // 2 and y == HEIGHT // 2
        if include_parity_mismatches or sample.get("parity_match", True):
            rows.append(row)
        if is_center:
            center = {
                "output_xy": [x, y],
                "ae_dx": ae_dx,
                "ae_dy": ae_dy,
                "native_sample_dx": native_dx,
                "native_sample_dy": native_dy,
                "native_field_dx": native.displacement[0],
                "native_field_dy": native.displacement[1],
                "vector_error": row["vector_error"],
                "parity_match": bool(sample.get("parity_match", True)),
            }

    if not rows:
        return {
            "case_id": case_id,
            "frame": frame_number,
            "status": "no_comparable_samples",
            "params": params_report(params),
            "notes": notes,
            "sample_count": 0,
            "skipped_transparent_count": skipped_transparent,
            "skipped_parity_mismatch_count": skipped_parity_mismatch,
            "skipped_native_oob_count": skipped_native_oob,
            "center": center,
        }

    vector_errors = [row["vector_error"] for row in rows]
    result = {
        "case_id": case_id,
        "frame": frame_number,
        "status": "ok",
        "params": params_report(params),
        "resolved_params": asdict(resolve_params(params)),
        "notes": notes,
        "sample_count": len(rows),
        "decoded_count": sum(1 for sample in samples if sample.get("decoded")),
        "skipped_transparent_count": skipped_transparent,
        "skipped_parity_mismatch_count": skipped_parity_mismatch,
        "skipped_native_oob_count": skipped_native_oob,
        "mean_signed_dx_error": mean(row["dx_error"] for row in rows),
        "mean_signed_dy_error": mean(row["dy_error"] for row in rows),
        "mean_abs_dx_error": mean(row["abs_dx_error"] for row in rows),
        "mean_abs_dy_error": mean(row["abs_dy_error"] for row in rows),
        "mean_abs_magnitude_error": mean(row["abs_magnitude_error"] for row in rows),
        "mean_vector_error": mean(vector_errors),
        "p95_vector_error": percentile(vector_errors, 0.95),
        "max_vector_error": max(vector_errors),
        "basis_scan": scan_signed_permutation_basis(rows),
        "center": center,
    }
    return result


def params_report(params: FieldParams) -> dict[str, Any]:
    report = asdict(params)
    report["offset"] = list(params.offset)
    return report


def select_cases(measurements: dict[str, Any], args: argparse.Namespace) -> list[dict[str, Any]]:
    cases = measurements.get("cases", [])
    by_id = {case["case_id"]: case for case in cases}
    if args.all_cases:
        return cases
    requested = args.case or REPRESENTATIVE_CASES
    missing = [case_id for case_id in requested if case_id not in by_id]
    if missing:
        raise SystemExit(f"Requested case(s) missing from measurements: {', '.join(missing)}")
    return [by_id[case_id] for case_id in requested]


def compare_measurements(
    measurements_path: Path,
    args: argparse.Namespace,
) -> dict[str, Any]:
    measurements = json.loads(measurements_path.read_text())
    case_results = []
    for case in select_cases(measurements, args):
        case_id = case["case_id"]
        for frame in case.get("frames", []):
            samples = load_samples_for_frame(frame, measurements_path, measurements)
            case_results.append(
                compare_frame(
                    case_id,
                    frame,
                    samples,
                    include_parity_mismatches=args.include_parity_mismatches,
                )
            )

    ok_results = [result for result in case_results if result.get("status") == "ok"]
    report = {
        "schema": "ae-native-renderer.turbulent-native-vector-comparison.v2",
        "measurements": str(measurements_path),
        "native_model": {
            "source": "Python mirror of crates/effects/src/turbulent_displace.rs",
            "sampler": "nearest_round",
            "comparison_vector": "native sampled source offset vs AE decoded source offset",
            "analysis": "signed permutation scan tests axis swap/flip hypotheses without formula tuning",
            "todo": "Replace mirror with an exported Rust telemetry API/CLI for arbitrary sample points.",
        },
        "selection": "all" if args.all_cases else [case["case_id"] for case in select_cases(measurements, args)],
        "include_parity_mismatches": args.include_parity_mismatches,
        "case_results": case_results,
        "summary": summarize_results(ok_results),
        "analysis": analyze_results(ok_results),
    }
    return report


def summarize_results(results: list[dict[str, Any]]) -> dict[str, Any]:
    if not results:
        return {"case_frame_count": 0}
    sample_count = sum(int(result["sample_count"]) for result in results)
    return {
        "case_frame_count": len(results),
        "sample_count": sample_count,
        "mean_abs_dx_error": weighted_mean(results, "mean_abs_dx_error"),
        "mean_abs_dy_error": weighted_mean(results, "mean_abs_dy_error"),
        "mean_abs_magnitude_error": weighted_mean(results, "mean_abs_magnitude_error"),
        "mean_vector_error": weighted_mean(results, "mean_vector_error"),
        "max_p95_vector_error": max(float(result["p95_vector_error"]) for result in results),
        "max_vector_error": max(float(result["max_vector_error"]) for result in results),
    }


def weighted_mean(results: list[dict[str, Any]], key: str) -> float:
    total_samples = sum(int(result["sample_count"]) for result in results)
    if total_samples == 0:
        return 0.0
    return sum(float(result[key]) * int(result["sample_count"]) for result in results) / total_samples


def case_group(case_id: str) -> str:
    if case_id.startswith("TD_AMOUNT_SWEEP"):
        return "amount"
    if case_id.startswith("TD_SIZE_SWEEP"):
        return "size"
    if case_id.startswith("TD_COMPLEXITY_SWEEP"):
        return "complexity"
    if case_id.startswith("TD_DISPLACEMENT_TYPE"):
        return "displacement_type"
    if case_id.startswith("TD_EVOLUTION_ANIM"):
        return "evolution_anim"
    if case_id.startswith("TD_EVOLUTION_STATIC"):
        return "evolution_static"
    if case_id.startswith("TD_SEED_SWEEP"):
        return "seed"
    if case_id.startswith("TD_RESIZE_LAYER"):
        return "resize_layer"
    if case_id.startswith("TD_SAMPLER_CHECK"):
        return "sampler_check"
    return "other"


def group_hint(group: str) -> str:
    hints = {
        "amount": "Use A010/A045/A100 after basis/sign is closer; A000 only validates pass-through.",
        "size": "Fit frequency/coordinate scale after type-1 basis; treat S001 as AE default-size fallback.",
        "complexity": "Fit octave weighting after amount/size/basis stop dominating the error.",
        "displacement_type": "Fit branch-specific bases after type 1; type 9 is a separate scalar vertical branch.",
        "evolution_anim": "Fit temporal phase/periodicity only after static field basis is closer.",
        "evolution_static": "Fit evolution phase/periodicity only after static field basis is closer.",
        "seed": "Fit seed coordinate offsets/phase after static type-1 basis and size scale.",
        "resize_layer": "Defer edge-policy tuning until interior field vectors are close.",
        "sampler_check": "Use this to validate decode/sampler trust before field formula tuning.",
    }
    return hints.get(group, "No group-specific hint.")


def summarize_result_group(group: str, results: list[dict[str, Any]]) -> dict[str, Any]:
    worst = max(results, key=lambda result: float(result["mean_vector_error"]))
    return {
        "group": group,
        "case_frame_count": len(results),
        "sample_count": sum(int(result["sample_count"]) for result in results),
        "mean_abs_dx_error": weighted_mean(results, "mean_abs_dx_error"),
        "mean_abs_dy_error": weighted_mean(results, "mean_abs_dy_error"),
        "mean_abs_magnitude_error": weighted_mean(results, "mean_abs_magnitude_error"),
        "mean_vector_error": weighted_mean(results, "mean_vector_error"),
        "max_p95_vector_error": max(float(result["p95_vector_error"]) for result in results),
        "worst_case_frame": {
            "case_id": worst["case_id"],
            "frame": worst["frame"],
            "mean_vector_error": worst["mean_vector_error"],
            "p95_vector_error": worst["p95_vector_error"],
        },
        "hint": group_hint(group),
    }


def is_type1_basis_scan_case(result: dict[str, Any]) -> bool:
    params = result.get("params") or {}
    case_id = str(result.get("case_id", ""))
    if int(params.get("displacement_type", 1)) != 1:
        return False
    if float(params.get("amount", 0.0)) <= 0.0:
        return False
    if params.get("resize_layer"):
        return False
    if case_id.startswith("TD_SAMPLER_CHECK"):
        return False
    return case_group(case_id) in {
        "amount",
        "size",
        "complexity",
        "evolution_static",
        "seed",
    } or case_id == "TD_DISPLACEMENT_TYPE_01"


def aggregate_basis_scans(results: list[dict[str, Any]]) -> dict[str, Any]:
    if not results:
        return {"case_frame_count": 0, "sample_count": 0, "candidates": []}

    by_candidate: dict[str, dict[str, Any]] = {}
    for result in results:
        scan = result.get("basis_scan") or {}
        for candidate in scan.get("candidates", []):
            name = candidate["candidate"]
            bucket = by_candidate.setdefault(
                name,
                {
                    "candidate": name,
                    "matrix": candidate["matrix"],
                    "sample_count": 0,
                    "weighted_mean_vector_error": 0.0,
                    "weighted_mean_abs_dx_error": 0.0,
                    "weighted_mean_abs_dy_error": 0.0,
                    "weighted_mean_cosine": 0.0,
                    "max_p95_vector_error": 0.0,
                    "max_vector_error": 0.0,
                    "weighted_opposite_direction_fraction": 0.0,
                },
            )
            samples = int(candidate["sample_count"])
            bucket["sample_count"] += samples
            bucket["weighted_mean_vector_error"] += float(candidate["mean_vector_error"]) * samples
            bucket["weighted_mean_abs_dx_error"] += float(candidate["mean_abs_dx_error"]) * samples
            bucket["weighted_mean_abs_dy_error"] += float(candidate["mean_abs_dy_error"]) * samples
            bucket["weighted_mean_cosine"] += float(candidate["mean_cosine"]) * samples
            bucket["weighted_opposite_direction_fraction"] += (
                float(candidate["opposite_direction_fraction"]) * samples
            )
            bucket["max_p95_vector_error"] = max(
                bucket["max_p95_vector_error"],
                float(candidate["p95_vector_error"]),
            )
            bucket["max_vector_error"] = max(
                bucket["max_vector_error"],
                float(candidate["max_vector_error"]),
            )

    candidates = []
    for bucket in by_candidate.values():
        samples = int(bucket["sample_count"])
        if samples == 0:
            continue
        candidates.append(
            {
                "candidate": bucket["candidate"],
                "matrix": bucket["matrix"],
                "sample_count": samples,
                "mean_vector_error": bucket["weighted_mean_vector_error"] / samples,
                "mean_abs_dx_error": bucket["weighted_mean_abs_dx_error"] / samples,
                "mean_abs_dy_error": bucket["weighted_mean_abs_dy_error"] / samples,
                "mean_cosine": bucket["weighted_mean_cosine"] / samples,
                "opposite_direction_fraction": (
                    bucket["weighted_opposite_direction_fraction"] / samples
                ),
                "max_p95_vector_error": bucket["max_p95_vector_error"],
                "max_vector_error": bucket["max_vector_error"],
            }
        )

    candidates.sort(key=lambda item: item["mean_vector_error"])
    identity = next((item for item in candidates if item["candidate"] == "identity"), None)
    best = candidates[0] if candidates else None
    improvement = (
        identity["mean_vector_error"] - best["mean_vector_error"]
        if identity is not None and best is not None
        else 0.0
    )
    return {
        "case_frame_count": len(results),
        "sample_count": sum(int(result["sample_count"]) for result in results),
        "cases": [f"{result['case_id']}@{result['frame']}" for result in results],
        "identity": identity,
        "best": best,
        "best_over_identity_mean_vector_improvement": improvement,
        "best_over_identity_mean_vector_improvement_fraction": (
            improvement / identity["mean_vector_error"]
            if identity is not None and identity["mean_vector_error"] > 1.1920929e-7
            else 0.0
        ),
        "candidates": candidates,
    }


def derive_tuning_order(
    groups: list[dict[str, Any]],
    type1_basis_scan: dict[str, Any],
) -> list[str]:
    order = []
    best = type1_basis_scan.get("best")
    identity = type1_basis_scan.get("identity")
    if best and identity:
        improvement_fraction = float(
            type1_basis_scan["best_over_identity_mean_vector_improvement_fraction"]
        )
        if improvement_fraction >= 0.20:
            order.append(
                "1. Test signed basis transform first: "
                f"{best['candidate']} lowers type-1 mean vector error from "
                f"{identity['mean_vector_error']:.3f} to {best['mean_vector_error']:.3f}."
            )
        else:
            order.append(
                "1. Do not tune amplitude first: signed flip/swap scan gives only "
                f"{improvement_fraction * 100.0:.1f}% type-1 improvement, so noise basis/origin "
                "must be fit before scale."
            )
    else:
        order.append("1. Collect type-1 basis scan data before formula tuning.")

    present_groups = {group["group"] for group in groups}
    if "amount" in present_groups:
        order.append("2. Fit amount scale on A010/A045/A100 after the type-1 basis decision.")
    if "size" in present_groups:
        order.append("3. Fit size/frequency on the size sweep, excluding S001 as a rejected-AE-control case.")
    if "displacement_type" in present_groups:
        order.append("4. Fit displacement type branches; keep type 9 separate from xy vector branches.")
    if "seed" in present_groups:
        order.append("5. Fit seed phase/coordinate offsets.")
    if "complexity" in present_groups:
        order.append("6. Fit complexity octave weighting.")
    if "evolution_static" in present_groups or "evolution_anim" in present_groups:
        order.append("7. Fit evolution phase and temporal period.")
    if "resize_layer" in present_groups:
        order.append("8. Revisit resize/pinning edges after interior field error is smaller.")
    return order


def analyze_results(results: list[dict[str, Any]]) -> dict[str, Any]:
    grouped: dict[str, list[dict[str, Any]]] = {}
    for result in results:
        grouped.setdefault(case_group(str(result["case_id"])), []).append(result)

    group_summaries = [
        summarize_result_group(group, group_results)
        for group, group_results in sorted(grouped.items())
    ]
    type1_results = [result for result in results if is_type1_basis_scan_case(result)]
    type1_basis_scan = aggregate_basis_scans(type1_results)
    return {
        "grouped_fitting_hints": group_summaries,
        "type1_signed_permutation_scan": type1_basis_scan,
        "next_tuning_order": derive_tuning_order(group_summaries, type1_basis_scan),
    }


def write_csv(report: dict[str, Any], output: Path) -> None:
    fieldnames = [
        "case_id",
        "frame",
        "group",
        "sample_count",
        "mean_abs_dx_error",
        "mean_abs_dy_error",
        "mean_abs_magnitude_error",
        "mean_vector_error",
        "p95_vector_error",
        "max_vector_error",
        "center_ae_dx",
        "center_ae_dy",
        "center_native_dx",
        "center_native_dy",
        "basis_best",
        "basis_best_mean_vector_error",
        "basis_identity_mean_vector_error",
        "skipped_parity_mismatch_count",
    ]
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames)
        writer.writeheader()
        for result in report["case_results"]:
            if result.get("status") != "ok":
                continue
            center = result.get("center") or {}
            scan = result.get("basis_scan") or {}
            best = scan.get("best") or {}
            identity = scan.get("identity") or {}
            writer.writerow(
                {
                    "case_id": result["case_id"],
                    "frame": result["frame"],
                    "group": case_group(str(result["case_id"])),
                    "sample_count": result["sample_count"],
                    "mean_abs_dx_error": result["mean_abs_dx_error"],
                    "mean_abs_dy_error": result["mean_abs_dy_error"],
                    "mean_abs_magnitude_error": result["mean_abs_magnitude_error"],
                    "mean_vector_error": result["mean_vector_error"],
                    "p95_vector_error": result["p95_vector_error"],
                    "max_vector_error": result["max_vector_error"],
                    "center_ae_dx": center.get("ae_dx"),
                    "center_ae_dy": center.get("ae_dy"),
                    "center_native_dx": center.get("native_sample_dx"),
                    "center_native_dy": center.get("native_sample_dy"),
                    "basis_best": best.get("candidate"),
                    "basis_best_mean_vector_error": best.get("mean_vector_error"),
                    "basis_identity_mean_vector_error": identity.get("mean_vector_error"),
                    "skipped_parity_mismatch_count": result["skipped_parity_mismatch_count"],
                }
            )


def print_table(report: dict[str, Any]) -> None:
    summary = report["summary"]
    if summary.get("case_frame_count", 0):
        print(
            "summary "
            f"case_frames={summary['case_frame_count']} "
            f"samples={summary['sample_count']} "
            f"mean_abs_dx={summary['mean_abs_dx_error']:.3f} "
            f"mean_abs_dy={summary['mean_abs_dy_error']:.3f} "
            f"mean_abs_mag={summary['mean_abs_magnitude_error']:.3f} "
            f"mean_vec={summary['mean_vector_error']:.3f} "
            f"max_p95_vec={summary['max_p95_vector_error']:.3f}"
        )
    else:
        print("summary case_frames=0 samples=0")

    print(
        "case frame samples mean_abs_dx mean_abs_dy mean_abs_mag "
        "mean_vec p95_vec center_ae_dxdy center_native_dxdy skipped_parity"
    )
    for result in report["case_results"]:
        if result.get("status") != "ok":
            print(f"{result['case_id']} {result['frame']} {result.get('status')}")
            continue
        center = result.get("center") or {}
        center_ae = f"({center.get('ae_dx', 0):.0f},{center.get('ae_dy', 0):.0f})"
        center_native = (
            f"({center.get('native_sample_dx', 0):.0f},"
            f"{center.get('native_sample_dy', 0):.0f})"
        )
        print(
            f"{result['case_id']} "
            f"{result['frame']} "
            f"{result['sample_count']} "
            f"{result['mean_abs_dx_error']:.3f} "
            f"{result['mean_abs_dy_error']:.3f} "
            f"{result['mean_abs_magnitude_error']:.3f} "
            f"{result['mean_vector_error']:.3f} "
            f"{result['p95_vector_error']:.3f} "
            f"{center_ae} "
            f"{center_native} "
            f"{result['skipped_parity_mismatch_count']}"
        )

    analysis = report.get("analysis") or {}
    type1_scan = analysis.get("type1_signed_permutation_scan") or {}
    best = type1_scan.get("best")
    identity = type1_scan.get("identity")
    if best and identity:
        print(
            "analysis type1_basis_scan "
            f"case_frames={type1_scan['case_frame_count']} "
            f"samples={type1_scan['sample_count']} "
            f"identity_mean_vec={identity['mean_vector_error']:.3f} "
            f"best={best['candidate']} "
            f"best_mean_vec={best['mean_vector_error']:.3f} "
            f"improvement="
            f"{type1_scan['best_over_identity_mean_vector_improvement_fraction'] * 100.0:.1f}%"
        )

    for group in analysis.get("grouped_fitting_hints", []):
        print(
            "analysis group "
            f"{group['group']} "
            f"case_frames={group['case_frame_count']} "
            f"samples={group['sample_count']} "
            f"mean_vec={group['mean_vector_error']:.3f} "
            f"max_p95={group['max_p95_vector_error']:.3f} "
            f"worst={group['worst_case_frame']['case_id']}@{group['worst_case_frame']['frame']}"
        )

    for item in analysis.get("next_tuning_order", []):
        print(f"analysis next {item}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description=(
            "Compare Round 5 AE Turbulent Displace vector measurements against "
            "the current native field model."
        )
    )
    parser.add_argument(
        "--measurements",
        type=Path,
        required=True,
        help="Path to turbulent_vector_measurements.json.",
    )
    parser.add_argument(
        "--case",
        action="append",
        help="Case id to compare. May be passed more than once. Defaults to representative cases.",
    )
    parser.add_argument(
        "--all-cases",
        action="store_true",
        help="Compare every mappable coordinate-field case in the measurements.",
    )
    parser.add_argument(
        "--include-parity-mismatches",
        action="store_true",
        help="Include decoded samples whose coordinate parity check failed.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Optional JSON report output path.",
    )
    parser.add_argument(
        "--csv-output",
        type=Path,
        default=None,
        help="Optional flat CSV output with case metrics and signed-basis scan winners.",
    )
    args = parser.parse_args()

    report = compare_measurements(args.measurements, args)
    print_table(report)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n")
        print(f"wrote {args.output}")
    if args.csv_output:
        write_csv(report, args.csv_output)
        print(f"wrote {args.csv_output}")


if __name__ == "__main__":
    main()
