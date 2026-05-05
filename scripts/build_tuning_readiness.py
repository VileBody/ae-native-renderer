#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from collections import defaultdict
from datetime import date
from pathlib import Path
from typing import Any


MODULE_CONFIG: dict[str, dict[str, Any]] = {
    "M01": {
        "title": "Layer activity / compositing",
        "lane": "global substrate",
        "state": "needs dedicated evidence",
        "primary_gate": "PRI_010/CMP_010 prove layer opacity/source-over/background behavior before effect tuning depends on it",
        "allowed_knobs": ["activity boundaries", "z-order application", "layer opacity scaling", "normal source-over implementation"],
        "forbidden": ["effect-specific alpha hacks", "text/effect formula changes"],
        "blockers": ["PRI_010 was not in the Step 4 integrated run; M19 premult/straight is still diagnostic-only"],
        "required_sidecars": ["metrics"],
    },
    "M02": {
        "title": "Footage/source-time sampling",
        "lane": "temporal substrate",
        "state": "diagnostic-ready",
        "primary_gate": "source_frame.index/time/subframe must match numbered-frame AE refs",
        "allowed_knobs": ["source time offset", "frame-index rounding", "activity-window boundary handling"],
        "forbidden": ["effect formula changes", "global alpha/composite policy changes"],
        "blockers": ["needs numbered-frame source passport on non-trivial source_start cases"],
        "required_sidecars": ["temporal"],
    },
    "M03": {
        "title": "2D transform matrix / sampler",
        "lane": "geometry substrate",
        "state": "needs dedicated evidence",
        "primary_gate": "coordinate-field transform passport proves matrix/pixel-center/OOB before Geometry2 or collapse tuning",
        "allowed_knobs": ["anchor/position/scale/rotation order", "inverse sampling convention", "pixel center", "edge/OOB policy"],
        "forbidden": ["Geometry2-specific parameter remaps", "collapse text raster hacks"],
        "blockers": ["no dedicated M03 case is measured in the Step 4 integrated run; use EFF_040-style coordinate-field probes"],
        "required_sidecars": ["metrics"],
    },
    "M04": {
        "title": "Keyframes / Bezier ease",
        "lane": "temporal math",
        "state": "evidence-ready, not tuning-ready",
        "primary_gate": "INT_020 keyframe_sample records improve without worsening TMP/Motion cases",
        "allowed_knobs": ["Bezier influence/speed mapping", "hold/linear/ease segment selection", "scalar/vector coercion"],
        "forbidden": ["changing source-frame quantization", "changing transform matrix convention"],
        "blockers": ["AE tangent/influence telemetry is still missing; final pixels alone are too indirect"],
        "required_sidecars": ["temporal"],
    },
    "M05": {
        "title": "Text glyph layout / sourceRect subset",
        "lane": "text",
        "state": "partial tuning-ready",
        "primary_gate": "text_passport.max_abs_delta and total_mismatches decrease on TXT_010..TXT_040",
        "allowed_knobs": ["font face mapping", "advance/bbox/baseline math", "multiline block placement", "composer whitespace handling"],
        "forbidden": ["tuning text pixels against full PNG before glyph/layout deltas improve"],
        "blockers": ["CoolType raster coverage/glyph ids are not yet probed; Montserrat instance mapping remains open"],
        "required_sidecars": ["text_telemetry", "text_passport"],
    },
    "M06": {
        "title": "Range Selector reveal",
        "lane": "text selector",
        "state": "evidence-ready, not tuning-ready",
        "primary_gate": "selector-unit boundaries/weights move independently from glyph layout drift",
        "allowed_knobs": ["BasedOn grouping", "Start/End boundary rounding", "shape/smoothness/randomize/wiggly semantics"],
        "forbidden": ["using glyph advance hacks to hide selector ordering bugs"],
        "blockers": ["needs focused AE selector boundary refs after M05 layout facts are stable"],
        "required_sidecars": ["text_telemetry", "text_passport"],
    },
    "M07": {
        "title": "Glyph animator transform/blur",
        "lane": "text animator",
        "state": "evidence-ready, not tuning-ready",
        "primary_gate": "per-glyph matrix/opacity/blur telemetry improves before final text blur pixels",
        "allowed_knobs": ["glyph transform origin", "opacity contribution", "blur radius/kernel approximation", "selector weighting"],
        "forbidden": ["global alpha/premult changes inside text animator code"],
        "blockers": ["text blur and opacity are substrate-dependent on M19; exact glyph metrics still depend on M05"],
        "required_sidecars": ["text_telemetry", "text_passport"],
    },
    "M08": {
        "title": "Expression selector bounce",
        "lane": "text expression selector",
        "state": "evidence-ready, not tuning-ready",
        "primary_gate": "bounce selector amount curve matches AE samples without perturbing static text layout",
        "allowed_knobs": ["delay", "frequency", "decay", "per-character phase/order"],
        "forbidden": ["arbitrary JS evaluator expansion in this pass"],
        "blockers": ["needs AE selector amount samples; current refs mostly expose layout/sourceRect fields"],
        "required_sidecars": ["text_telemetry", "text_passport"],
    },
    "M09": {
        "title": "Property expression subset",
        "lane": "expressions",
        "state": "evidence-ready, not tuning-ready",
        "primary_gate": "EXP_010 expression telemetry and final motion improve without changing timeline sampling",
        "allowed_knobs": ["named evaluator traits", "scalar/vector coercion", "thisLayer/thisComp access", "wobble envelope parameters"],
        "forbidden": ["unbounded ExtendScript execution", "changing layer time globally"],
        "blockers": ["needs property sample refs for generated expressions and expression-selector amount curves"],
        "required_sidecars": ["expression"],
    },
    "M10": {
        "title": "Drop Shadow / Box Blur dependency",
        "lane": "effects",
        "state": "blocked on M19 + intermediate refs",
        "primary_gate": "EFF_010/EFF_030/EFF_070 improve on rgb_under_alpha_policy with stable alpha stats",
        "allowed_knobs": ["shadow offset sign/truncation", "softness-to-radius mapping", "box blur passes", "shadow mask construction"],
        "forbidden": ["raw RGBA-only tuning", "global alpha policy edits inside Drop Shadow"],
        "blockers": ["Drop Shadow final composite path and alpha policy are not locked"],
        "required_sidecars": ["effects_debug"],
    },
    "M11": {
        "title": "Glow",
        "lane": "effects",
        "state": "blocked on M19 + enum/probe refs",
        "primary_gate": "EFF_020/EFF_070 rgb_under_alpha_policy improves after BasedOn/source threshold is locked",
        "allowed_knobs": ["Glow Based On enum", "threshold source", "blur radius/intensity mapping", "blend/composite route"],
        "forbidden": ["tuning intensity before BasedOn enum is resolved", "raw RGBA-only tuning"],
        "blockers": ["Glow Based On enum and IR_GaussianBlur/composite route need direct evidence"],
        "required_sidecars": ["effects_debug"],
    },
    "M12": {
        "title": "Geometry2",
        "lane": "warps",
        "state": "evidence-ready, needs isolated run",
        "primary_gate": "coordinate-field UV/matrix sidecars match before STK_030 final pixels are tuned",
        "allowed_knobs": ["AE property mapping", "matrix order", "sampler quality", "edge/OOB policy"],
        "forbidden": ["tuning Geometry2 from STK_030 final PNG only", "changing M03 layer transform semantics"],
        "blockers": ["EFF_040 isolated coordinate-field case was not in the Step 4 integrated run"],
        "required_sidecars": ["effects_debug", "adjustment"],
    },
    "M13": {
        "title": "Minimax",
        "lane": "morphology",
        "state": "evidence-ready, needs isolated refs",
        "primary_gate": "EFF_050/STK_020/STK_030 direction/radius checkpoints improve before stack pixels",
        "allowed_knobs": ["operation/channel enum", "direction enum", "fractional radius", "Don't Shrink Edges behavior"],
        "forbidden": ["Turbulent/Geometry changes in a Minimax patch"],
        "blockers": ["fractional radius and Direction enum AE refs are still missing from current run"],
        "required_sidecars": ["effects_debug", "adjustment"],
    },
    "M14": {
        "title": "Turbulent Displace",
        "lane": "procedural field",
        "state": "evidence-ready, not tuning-ready",
        "primary_gate": "field_hash/sample grid moves toward AE coordinate-field refs before final pixels",
        "allowed_knobs": ["FracAll/Frac1D state model", "evolution/size/amount mapping", "octaves/complexity", "pinning/OOB"],
        "forbidden": ["final PNG-only tuning", "changing Posterize Time routing in Turbulent patch"],
        "blockers": ["needs field-level AE refs or kernel-derived field maps; EFF_060 was not in integrated run"],
        "required_sidecars": ["effects_debug"],
    },
    "M15": {
        "title": "Posterize Time",
        "lane": "temporal routing",
        "state": "partial tuning-ready",
        "primary_gate": "TMP_020 remains exact; STK_030 source/effect times show correct bucket/live split",
        "allowed_knobs": ["bucket boundary rounding", "fixed16 fps conversion", "layer/source/effect time routing"],
        "forbidden": ["posterizing downstream effects that AE evaluates at live comp time"],
        "blockers": ["boundary epsilon and adjustment stack ordering still need AE micro refs"],
        "required_sidecars": ["temporal"],
    },
    "M16": {
        "title": "Adjustment stack order",
        "lane": "render graph/effects",
        "state": "evidence-ready, not tuning-ready",
        "primary_gate": "per-effect input/output hashes localize first divergent adjustment effect in STK_030",
        "allowed_knobs": ["lower-stack resampling point", "effect application order", "adjustment input snapshot boundaries"],
        "forbidden": ["changing individual effect formulas while validating stack order"],
        "blockers": ["needs AE-side intermediate/checkpoint refs for STK_030"],
        "required_sidecars": ["effects_debug", "adjustment"],
    },
    "M17": {
        "title": "Collapse transformations / precomp graph",
        "lane": "graph/collapse",
        "state": "evidence-ready, not tuning-ready",
        "primary_gate": "GPH_010 matrix_report and deferred-raster checkpoints match collapsed/noncollapsed AE refs",
        "allowed_knobs": ["matrix pushdown", "collapse boundary selection", "text/vector deferred rasterization"],
        "forbidden": ["raster sharpness hacks before deferred-raster contract is known"],
        "blockers": ["true AE deferred text/vector rasterization refs are still missing"],
        "required_sidecars": ["collapse"],
    },
    "M18": {
        "title": "Motion blur",
        "lane": "temporal sampling",
        "state": "evidence-ready, not tuning-ready",
        "primary_gate": "TMP_030 shutter sample times/weights match AE before accumulation/composite tuning",
        "allowed_knobs": ["sample endpoints", "weight distribution", "shutter phase/angle mapping", "static-layer skip"],
        "forbidden": ["changing transform math while tuning motion accumulation"],
        "blockers": ["AE shutter sample telemetry/weights are missing; premult accumulation depends on M19"],
        "required_sidecars": ["temporal"],
    },
    "M19": {
        "title": "Color/alpha/sampling/gamma substrate",
        "lane": "global substrate",
        "state": "first tuning target",
        "primary_gate": "alpha_policy_diagnostics stable; rgb_under_alpha_policy is used for effect tuning until premult is locked",
        "allowed_knobs": ["straight/premult conversion", "background alpha normalization", "source-over math", "gamma/color-space decision", "sampler edge policy"],
        "forbidden": ["module-specific formula hacks to compensate global substrate drift"],
        "blockers": ["premult/straight contract is still diagnostic-only"],
        "required_sidecars": ["metrics"],
        "case_selector": "all_measured",
    },
}


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as f:
        return json.load(f)


def jsonl_count(path: Path) -> int:
    if not path.exists():
        return 0
    with path.open("r", encoding="utf-8") as f:
        return sum(1 for line in f if line.strip())


def metric(summary: dict[str, Any], path: str) -> Any:
    cur: Any = summary
    for part in path.split("."):
        if not isinstance(cur, dict):
            return None
        cur = cur.get(part)
    return cur


def number(value: Any) -> float | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return float(value)
    return None


def fmt_num(value: Any, digits: int = 4) -> str:
    n = number(value)
    if n is None:
        return "-"
    if digits <= 0:
        return str(int(round(n)))
    if abs(n) >= 100:
        return f"{n:.1f}"
    text = f"{n:.{digits}f}".rstrip("0").rstrip(".")
    return text if text else "0"


def report_case_map(report: dict[str, Any]) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for case in report.get("cases", []):
        case_id = case.get("case") or case.get("id")
        if case_id:
            out[str(case_id)] = case
    return out


def manifest_cases_by_module(manifest: dict[str, Any]) -> dict[str, list[str]]:
    out: dict[str, list[str]] = defaultdict(list)
    for case in manifest.get("cases", []):
        case_id = case["id"]
        for module in case.get("modules", []):
            out[module].append(case_id)
    return dict(out)


def measured_cases_by_module(cases: dict[str, dict[str, Any]]) -> dict[str, list[str]]:
    out: dict[str, list[str]] = defaultdict(list)
    for case_id, case in cases.items():
        for module in case.get("modules", []):
            out[module].append(case_id)
    return dict(out)


def sidecar_inventory(out_root: Path, case_id: str) -> dict[str, Any]:
    case_dir = out_root / case_id
    effects_dir = out_root / "effects_debug" / case_id
    effects_debug = sorted(effects_dir.rglob("*.json")) if effects_dir.exists() else []
    inventory = {
        "metrics": (case_dir / "metrics.json").exists(),
        "temporal": {
            "path": str(case_dir / "temporal_telemetry.jsonl"),
            "records": jsonl_count(case_dir / "temporal_telemetry.jsonl"),
        },
        "text_telemetry": {
            "path": str(case_dir / "text_telemetry.jsonl"),
            "records": jsonl_count(case_dir / "text_telemetry.jsonl"),
        },
        "text_passport": (case_dir / "text_passport_comparison.json").exists(),
        "expression": {
            "path": str(case_dir / "expression_telemetry.jsonl"),
            "records": jsonl_count(case_dir / "expression_telemetry.jsonl"),
        },
        "collapse": {
            "path": str(case_dir / "collapse_telemetry.jsonl"),
            "records": jsonl_count(case_dir / "collapse_telemetry.jsonl"),
        },
        "adjustment": {
            "path": str(case_dir / "adjustment_effects.jsonl"),
            "records": jsonl_count(case_dir / "adjustment_effects.jsonl"),
        },
        "effects_debug": {
            "dir": str(effects_dir),
            "files": len(effects_debug),
        },
        "pngs": {
            "ae": len(list((case_dir / "ae").glob("*.png"))) if (case_dir / "ae").exists() else 0,
            "native": len(list((case_dir / "native").glob("*.png"))) if (case_dir / "native").exists() else 0,
            "diff": len(list((case_dir / "diff").glob("*.png"))) if (case_dir / "diff").exists() else 0,
        },
    }
    return inventory


def sidecar_ok(inventory: dict[str, Any], name: str) -> bool:
    if name == "metrics":
        return bool(inventory["metrics"])
    if name == "temporal":
        return inventory["temporal"]["records"] > 0
    if name == "text_telemetry":
        return inventory["text_telemetry"]["records"] > 0
    if name == "text_passport":
        return bool(inventory["text_passport"])
    if name == "expression":
        return inventory["expression"]["records"] > 0
    if name == "collapse":
        return inventory["collapse"]["records"] > 0
    if name == "adjustment":
        return inventory["adjustment"]["records"] > 0
    if name == "effects_debug":
        return inventory["effects_debug"]["files"] > 0
    return False


def case_summary(case: dict[str, Any], inventory: dict[str, Any]) -> dict[str, Any]:
    summary = case.get("summary", {})
    text = summary.get("text_passport", {})
    return {
        "case": case.get("case"),
        "modules": case.get("modules", []),
        "frames": summary.get("frames"),
        "ok": case.get("ok"),
        "status": case.get("status"),
        "mean_abs_diff": summary.get("mean_abs_diff"),
        "max_abs_diff": summary.get("max_abs_diff"),
        "rgb_under_alpha_policy": metric(summary, "alpha_policy_diagnostics.mean_abs_diff.rgb_straight_source_over_ae_background"),
        "alpha_mean": metric(summary, "metrics.alpha.mean_abs_diff"),
        "text_passport": {
            "compared_frames": text.get("compared_frames", 0),
            "missing_reference_frames": text.get("missing_reference_frames", []),
            "total_mismatches": text.get("total_mismatches", 0),
            "max_abs_delta": text.get("max_abs_delta", 0.0),
            "first_mismatch": text.get("first_mismatch"),
        },
        "sidecars": inventory,
    }


def build_packets(
    manifest: dict[str, Any],
    report: dict[str, Any],
    out_root: Path,
) -> dict[str, Any]:
    cases = report_case_map(report)
    manifest_by_module = manifest_cases_by_module(manifest)
    measured_by_module = measured_cases_by_module(cases)
    all_measured = sorted(cases)

    inventories = {case_id: sidecar_inventory(out_root, case_id) for case_id in cases}
    case_summaries = {
        case_id: case_summary(case, inventories[case_id]) for case_id, case in cases.items()
    }

    packets: list[dict[str, Any]] = []
    for module_id in sorted(MODULE_CONFIG):
        config = MODULE_CONFIG[module_id]
        measured = all_measured if config.get("case_selector") == "all_measured" else sorted(
            measured_by_module.get(module_id, [])
        )
        manifest_cases = sorted(manifest_by_module.get(module_id, []))
        missing_from_run = [case for case in manifest_cases if case not in measured]
        required = config.get("required_sidecars", [])

        evidence_by_case = {}
        missing_required_by_case = {}
        for case_id in measured:
            inv = inventories[case_id]
            evidence_by_case[case_id] = inv
            missing_required = [name for name in required if not sidecar_ok(inv, name)]
            if missing_required:
                missing_required_by_case[case_id] = missing_required

        packet = {
            "module": module_id,
            "title": config["title"],
            "lane": config["lane"],
            "state": config["state"],
            "primary_gate": config["primary_gate"],
            "allowed_knobs": config["allowed_knobs"],
            "forbidden": config["forbidden"],
            "blockers": config["blockers"],
            "manifest_cases": manifest_cases,
            "measured_cases": measured,
            "missing_manifest_cases_from_step4_run": missing_from_run,
            "required_sidecars": required,
            "missing_required_sidecars_by_case": missing_required_by_case,
            "case_summaries": [case_summaries[case_id] for case_id in measured],
        }
        packets.append(packet)

    return {
        "schema": "ae-native-renderer.tuning-readiness.v1",
        "generated_on": date.today().isoformat(),
        "source_report": str(out_root / "report.json"),
        "source_pack": "fixtures/ae_conformance_pack/manifest.json",
        "step": "4.5",
        "purpose": "Lock module evidence, sidecars, baselines, and guardrails before AE formula tuning.",
        "report_summary": report.get("summary", {}),
        "packets": packets,
    }


def md_list(values: list[str]) -> str:
    return "<br>".join(values) if values else "-"


def sidecar_badge(summary: dict[str, Any]) -> str:
    sidecars = summary["sidecars"]
    bits = []
    if sidecars["temporal"]["records"]:
        bits.append(f"temporal:{sidecars['temporal']['records']}")
    if sidecars["text_telemetry"]["records"]:
        bits.append(f"text:{sidecars['text_telemetry']['records']}")
    if sidecars["expression"]["records"]:
        bits.append(f"expr:{sidecars['expression']['records']}")
    if sidecars["collapse"]["records"]:
        bits.append(f"collapse:{sidecars['collapse']['records']}")
    if sidecars["adjustment"]["records"]:
        bits.append(f"adjust:{sidecars['adjustment']['records']}")
    if sidecars["effects_debug"]["files"]:
        bits.append(f"effects:{sidecars['effects_debug']['files']}")
    if sidecars["text_passport"]:
        bits.append("text_passport")
    return ", ".join(bits) if bits else "-"


def render_markdown(data: dict[str, Any]) -> str:
    lines: list[str] = []
    lines.append("# Step 4.5 Tuning Readiness / Evidence Lock")
    lines.append("")
    lines.append(f"Generated: {data['generated_on']}.")
    lines.append("")
    lines.append("Step 4.5 is the bridge between broad instrumentation and real formula tuning.")
    lines.append("It freezes the current conformance evidence, names the allowed knobs per module,")
    lines.append("and records which substrate facts must not be guessed from final PNGs.")
    lines.append("")
    lines.append("Source artifacts:")
    lines.append("")
    lines.append(f"- `{data['source_report']}`")
    lines.append("- `fixtures/ae_conformance_pack/manifest.json`")
    lines.append("- `docs/reverse_engineering/MATH_CONTRACTS_GUARDRAILS.md`")
    lines.append("")
    lines.append("## Global Locks")
    lines.append("")
    lines.append("| Contract | Step 4.5 decision | Guardrail |")
    lines.append("| --- | --- | --- |")
    lines.append("| Alpha/premult | Diagnostic-only; `rgb_straight_source_over_ae_background` is the preferred effects metric until M19 is locked. | Do not tune Drop Shadow/Glow/blur from raw RGBA alone. |")
    lines.append("| Text | SourceRect-based AE refs are valid for layout/advance/bbox tuning. | Do not claim 1:1 CoolType raster coverage until deeper glyph-id/coverage probes exist. |")
    lines.append("| Time | TMP_010/TMP_020 are exact in current run, but boundary semantics are not parity locked. | Do not change downstream adjustment timing while tuning Posterize buckets. |")
    lines.append("| Warps | STK_030 has native checkpoints for Geometry2/Minimax/Turbulent. | Do not tune field math from final stack pixels without coordinate/field refs. |")
    lines.append("| Collapse | Matrix reports exist; deferred text/vector raster is not locked. | Do not hide collapsed-text blur with raster sharpness hacks. |")
    lines.append("")
    lines.append("## Module Gateboard")
    lines.append("")
    lines.append("| Module | State | Measured cases | Missing pack cases | Primary gate | Blocker |")
    lines.append("| --- | --- | --- | --- | --- | --- |")
    for packet in data["packets"]:
        lines.append(
            "| {module} | {state} | {measured} | {missing} | {gate} | {blocker} |".format(
                module=f"`{packet['module']}` {packet['title']}",
                state=packet["state"],
                measured=md_list([f"`{case}`" for case in packet["measured_cases"]]),
                missing=md_list([f"`{case}`" for case in packet["missing_manifest_cases_from_step4_run"]]),
                gate=packet["primary_gate"],
                blocker=md_list(packet["blockers"]),
            )
        )
    lines.append("")
    lines.append("## Tuning Packets")
    lines.append("")
    for packet in data["packets"]:
        lines.append(f"### `{packet['module']}` {packet['title']}")
        lines.append("")
        lines.append(f"- Lane: {packet['lane']}")
        lines.append(f"- State: {packet['state']}")
        lines.append(f"- Primary gate: {packet['primary_gate']}")
        lines.append(f"- Allowed knobs: {', '.join(packet['allowed_knobs'])}")
        lines.append(f"- Forbidden in this module: {', '.join(packet['forbidden'])}")
        lines.append(f"- Blockers: {', '.join(packet['blockers'])}")
        if packet["missing_required_sidecars_by_case"]:
            lines.append(f"- Missing required sidecars: `{json.dumps(packet['missing_required_sidecars_by_case'], sort_keys=True)}`")
        lines.append("")
        lines.append("| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |")
        lines.append("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |")
        for case in packet["case_summaries"]:
            text = case["text_passport"]
            lines.append(
                "| `{case}` | {frames} | {mean} | {rgb} | {alpha} | {maxd} | {mismatches} | {text_delta} | {evidence} |".format(
                    case=case["case"],
                    frames=case.get("frames") if case.get("frames") is not None else "-",
                    mean=fmt_num(case.get("mean_abs_diff")),
                    rgb=fmt_num(case.get("rgb_under_alpha_policy")),
                    alpha=fmt_num(case.get("alpha_mean")),
                    maxd=fmt_num(case.get("max_abs_diff"), 0),
                    mismatches=text.get("total_mismatches", 0),
                    text_delta=fmt_num(text.get("max_abs_delta")),
                    evidence=sidecar_badge(case),
                )
            )
        lines.append("")
    lines.append("## Step 5 Entry Criteria")
    lines.append("")
    lines.append("Formula tuning may start for a module only when:")
    lines.append("")
    lines.append("1. The packet names the exact cases and sidecars that will be used.")
    lines.append("2. The patch declares which knobs it is allowed to change.")
    lines.append("3. The patch compares against this Step 4.5 baseline and reports deltas.")
    lines.append("4. Any substrate dependency is either locked first or explicitly excluded from the gate.")
    lines.append("5. Final PNG improvements are accepted only when the first divergent primitive is known.")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description="Build Step 4.5 tuning readiness packets.")
    parser.add_argument("--pack", type=Path, default=Path("fixtures/ae_conformance_pack"))
    parser.add_argument(
        "--report",
        type=Path,
        default=Path("target/ae_agents/step4_math_after_current/report.json"),
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        default=Path("fixtures/ae_conformance_pack/analysis/tuning_readiness_step4_5.json"),
    )
    parser.add_argument(
        "--md-out",
        type=Path,
        default=Path("docs/phase_reports/STEP_4_5_TUNING_READINESS_20260505.md"),
    )
    args = parser.parse_args()

    manifest = load_json(args.pack / "manifest.json")
    report = load_json(args.report)
    out_root = args.report.parent
    data = build_packets(manifest, report, out_root)

    args.json_out.parent.mkdir(parents=True, exist_ok=True)
    args.json_out.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    args.md_out.parent.mkdir(parents=True, exist_ok=True)
    args.md_out.write_text(render_markdown(data), encoding="utf-8")

    print(f"wrote {args.json_out}")
    print(f"wrote {args.md_out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
