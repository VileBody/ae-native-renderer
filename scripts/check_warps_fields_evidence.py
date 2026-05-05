#!/usr/bin/env python3
"""Gate isolated Geometry2/Turbulent evidence sidecars.

This intentionally checks coordinate/field telemetry, not final PNG metrics.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


EXPECTED_EFF_040_FRAMES = [0]
EXPECTED_EFF_060_FRAMES = [0, 15, 30, 45]
EXPECTED_STK_030_FRAMES = [0, 1, 5, 10, 15, 20, 30, 45, 59]
EXPECTED_STK_030_ORDER = [
    "ADBE Geometry2",
    "ADBE Posterize Time",
    "ADBE Minimax",
    "ADBE Turbulent Displace",
]


class EvidenceError(RuntimeError):
    pass


def load_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise EvidenceError(f"missing {path}") from exc


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        raise EvidenceError(f"missing {path}")
    records = []
    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        try:
            records.append(json.loads(line))
        except json.JSONDecodeError as exc:
            raise EvidenceError(f"{path}:{line_no} is not valid JSONL") from exc
    return records


def case_map(report: dict[str, Any]) -> dict[str, dict[str, Any]]:
    cases = {}
    for case in report.get("cases", []):
        case_id = case.get("case") or case.get("id")
        if case_id:
            cases[str(case_id)] = case
    return cases


def sidecar_payload(value: dict[str, Any]) -> dict[str, Any]:
    trace = value.get("trace")
    if isinstance(trace, dict):
        return trace
    return value


def read_effect_sidecars(out_root: Path, case_id: str, frame: int) -> list[tuple[Path, dict[str, Any]]]:
    paths: list[Path] = []
    for directory in [
        out_root / "effects_debug" / case_id / str(frame),
        out_root / case_id / "effects_debug" / f"frame_{frame:05}",
    ]:
        if directory.exists():
            paths.extend(sorted(directory.glob("*.json")))

    sidecars = []
    for path in paths:
        sidecars.append((path, sidecar_payload(load_json(path))))
    return sidecars


def require(condition: bool, message: str) -> None:
    if not condition:
        raise EvidenceError(message)


def require_schema(
    out_root: Path,
    case_id: str,
    frame: int,
    schema: str,
) -> tuple[Path, dict[str, Any]]:
    for path, payload in read_effect_sidecars(out_root, case_id, frame):
        if payload.get("schema") == schema:
            return path, payload
    raise EvidenceError(f"{case_id} frame {frame} is missing sidecar schema {schema}")


def validate_geometry(out_root: Path) -> dict[str, Any]:
    frames = []
    for frame in EXPECTED_EFF_040_FRAMES:
        path, payload = require_schema(
            out_root,
            "EFF_040",
            frame,
            "ae-native-renderer.geometry2-debug.v1",
        )
        mapping = payload.get("property_mapping", {})
        expected_roles = {
            "0003": "uniform_scale_fallback",
            "0004": "scale_height",
            "0008": "rotation_degrees",
        }
        for key, role in expected_roles.items():
            entry = mapping.get(key, {})
            require(
                entry.get("present") is True,
                f"EFF_040 frame {frame} property_mapping.{key} is not marked present",
            )
            require(
                entry.get("native_role") == role,
                f"EFF_040 frame {frame} property_mapping.{key}.native_role != {role}",
            )
            require(
                key in payload.get("raw_params", {}),
                f"EFF_040 frame {frame} raw_params is missing {key}",
            )
        require(
            len(payload.get("forward_matrix", [])) == 3
            and len(payload.get("inverse_matrix", [])) == 3,
            f"EFF_040 frame {frame} missing 3x3 matrix checkpoints",
        )
        samples = payload.get("samples", [])
        require(len(samples) >= 9, f"EFF_040 frame {frame} has fewer than 9 UV samples")
        for field in ["output_xy", "source_uv", "sample_xy", "out_of_bounds"]:
            require(field in samples[0], f"EFF_040 frame {frame} sample missing {field}")
        frames.append(
            {
                "frame": frame,
                "sidecar": str(path),
                "scale": payload.get("resolved", {}).get("scale"),
                "rotation": payload.get("resolved", {}).get("rotation"),
                "samples": len(samples),
                "out_of_bounds_count": payload.get("out_of_bounds_count"),
            }
        )
    return {"case": "EFF_040", "frames": frames}


def validate_turbulent(out_root: Path) -> dict[str, Any]:
    frames = []
    field_hashes = set()
    evolutions = []
    for frame in EXPECTED_EFF_060_FRAMES:
        path, payload = require_schema(
            out_root,
            "EFF_060",
            frame,
            "ae-native-renderer.turbulent-displace-field.v1",
        )
        for field in ["ae_wrapper", "field_state", "field_hash", "field_hash_u64"]:
            require(field in payload, f"EFF_060 frame {frame} sidecar missing {field}")
        field_state = payload["field_state"]
        wrapper = payload["ae_wrapper"]
        require(
            field_state.get("tuning_guardrail") == "do_not_tune_from_final_png_only",
            f"EFF_060 frame {frame} lost field-first tuning guardrail",
        )
        require(
            field_state.get("dispatch_path") == wrapper.get("kernel_path"),
            f"EFF_060 frame {frame} field_state dispatch_path does not match ae_wrapper kernel_path",
        )
        samples = payload.get("samples", [])
        require(len(samples) >= 9, f"EFF_060 frame {frame} has fewer than 9 field samples")
        for field in ["noise", "displacement", "source_uv", "sample_xy", "out_of_bounds"]:
            require(field in samples[0], f"EFF_060 frame {frame} sample missing {field}")
        field_hashes.add(payload.get("field_hash"))
        evolution = payload.get("resolved", {}).get("evolution")
        evolutions.append(evolution)
        frames.append(
            {
                "frame": frame,
                "sidecar": str(path),
                "evolution": evolution,
                "field_hash": payload.get("field_hash"),
                "samples": len(samples),
                "out_of_bounds_count": payload.get("out_of_bounds_count"),
            }
        )
    require(len(field_hashes) > 1, "EFF_060 field_hash is flat across all evidence frames")
    require(len(set(evolutions)) > 1, "EFF_060 resolved evolution is flat across evidence frames")
    return {"case": "EFF_060", "frames": frames}


def validate_stack(out_root: Path) -> dict[str, Any]:
    records = load_jsonl(out_root / "STK_030" / "adjustment_effects.jsonl")
    by_frame: dict[int, list[dict[str, Any]]] = {}
    for record in records:
        frame = record.get("frame")
        if isinstance(frame, int):
            by_frame.setdefault(frame, []).append(record)

    frames = []
    for frame in EXPECTED_STK_030_FRAMES:
        frame_records = sorted(by_frame.get(frame, []), key=lambda item: item.get("effect_index", -1))
        order = [record.get("match_name") for record in frame_records]
        require(
            order == EXPECTED_STK_030_ORDER,
            f"STK_030 frame {frame} adjustment order {order} != {EXPECTED_STK_030_ORDER}",
        )
        geometry = require_schema(
            out_root,
            "STK_030",
            frame,
            "ae-native-renderer.geometry2-debug.v1",
        )[1]
        turbulent = require_schema(
            out_root,
            "STK_030",
            frame,
            "ae-native-renderer.turbulent-displace-field.v1",
        )[1]
        require("property_mapping" in geometry, f"STK_030 frame {frame} Geometry2 trace missing property_mapping")
        require("field_state" in turbulent, f"STK_030 frame {frame} Turbulent trace missing field_state")
        frames.append(
            {
                "frame": frame,
                "order": order,
                "geometry_effect_time": next(
                    record["param_time"] for record in frame_records if record["match_name"] == "ADBE Geometry2"
                ),
                "turbulent_effect_time": next(
                    record["param_time"]
                    for record in frame_records
                    if record["match_name"] == "ADBE Turbulent Displace"
                ),
            }
        )
    return {"case": "STK_030", "frames": frames}


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Validate isolated EFF_040/EFF_060 warps/fields evidence and STK_030 stack checkpoints."
    )
    parser.add_argument("--out", type=Path, required=True, help="conformance-pack output directory")
    parser.add_argument("--write-json", type=Path, help="optional path for the validation summary JSON")
    args = parser.parse_args()

    try:
        report = load_json(args.out / "report.json")
        cases = case_map(report)
        for case_id in ["EFF_040", "EFF_060", "STK_030"]:
            require(case_id in cases, f"report.json is missing required case {case_id}")
            require(cases[case_id].get("ok") is True, f"{case_id} did not complete successfully")

        summary = {
            "schema": "ae-native-renderer.warps-fields-evidence-check.v1",
            "out": str(args.out),
            "ok": True,
            "geometry2": validate_geometry(args.out),
            "turbulent_displace": validate_turbulent(args.out),
            "stk_030": validate_stack(args.out),
        }
    except EvidenceError as exc:
        print(f"warps-fields evidence check failed: {exc}", file=sys.stderr)
        return 1

    text = json.dumps(summary, indent=2, sort_keys=True)
    if args.write_json:
        args.write_json.parent.mkdir(parents=True, exist_ok=True)
        args.write_json.write_text(text + "\n", encoding="utf-8")
    print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
