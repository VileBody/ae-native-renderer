#!/usr/bin/env python3
"""Build a compact intake report for AE reverse/probe artifacts."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]


REQUIREMENTS: list[dict[str, Any]] = [
    {
        "id": "M19_core_alpha_composite",
        "goal": "Lock straight/premult alpha, hidden RGB, normal blend, opacity, and source-over metric normalization.",
        "questions": [
            "What do GF::Composite bool flags mean?",
            "What alpha type enters/exits Transfer/Composite/AlphaGain?",
            "When does AE preserve hidden RGB under alpha zero?",
        ],
        "binaries": [
            "target/reverse/ae_2026/core_composite_alpha/GPUFoundation.dll",
            "target/reverse/ae_2026/core_composite_alpha/RendererCPU.dll",
            "target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/Blend.aex",
            "target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/SolidComposite.aex",
            "target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/Unmult.aex",
            "target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/Unmultiply.aex",
        ],
        "predecode_tasks": ["core_alpha_gpufoundation"],
        "ae_artifacts": [
            "fixtures/ae_conformance_pack/manifest.json",
            "fixtures/ae_conformance_pack/ae_goldens/metadata/tiff_png_summary.json",
            "fixtures/ae_conformance_pack/ae_goldens/downloads/ae_conformance_goldens_png_tiff_ae_conformance_clean_cases_85_20260503_170505_85.zip",
        ],
    },
    {
        "id": "M10_M11_M13_glow_shadow_blur",
        "goal": "Lock Drop Shadow alpha mask/composite, Glow mask source/composite, and shared blur alpha options.",
        "questions": [
            "Does Drop Shadow blur alpha only and where does color enter?",
            "Does Glow threshold use RGB, alpha, luma, or premult source?",
            "Which ImageRenderer/GPUFoundation blur/composite path is used?",
        ],
        "binaries": [
            "target/reverse/ae_2026/effects_blur_glow_shadow/Box_Blur.aex",
            "target/reverse/ae_2026/effects_blur_glow_shadow/Drop_Shadow.aex",
            "target/reverse/ae_2026/effects_blur_glow_shadow/Glow.aex",
            "target/reverse/ae_2026/core_composite_alpha/ImageRenderer.dll",
            "target/reverse/ae_2026/core_composite_alpha/GPUFoundation.dll",
        ],
        "predecode_tasks": [
            "blur_gpufoundation_kernels",
            "box_blur_aex",
            "drop_shadow_aex",
            "glow_aex",
            "imagerenderer_gaussian_composite",
        ],
        "ae_artifacts": [
            "fixtures/ae_probe_pack/glow_shadow/manifest.json",
            "fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/glow_shadow/ae_probe_outputs/png",
            "fixtures/ae_probe_pack/downloads/ae_probe_round5_85_20260504_011829_tiff_png_outputs_85.zip",
        ],
    },
    {
        "id": "M12_minimax",
        "goal": "Lock operation/channel/direction enum mapping, radius rounding, edge policy, and bpc path.",
        "questions": [
            "Are radius values floored, ceiled, rounded, or fractional?",
            "How does don't-shrink-edges alter sampling outside source bounds?",
            "Are 8/16/32 bpc callbacks equivalent after normalization?",
        ],
        "binaries": ["target/reverse/ae_2026/effects_temporal_noise_distort/Minimax.aex"],
        "predecode_tasks": ["minimax_aex"],
        "ae_artifacts": [
            "fixtures/ae_probe_pack/minimax/manifest.json",
            "fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/minimax/ae_goldens/metadata/minimax_measurements.json",
            "fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/minimax/ae_goldens/metadata/minimax_property_dump.json",
            "fixtures/ae_probe_pack/downloads/ae_probe_round5_85_20260504_011829_tiff_png_outputs_85.zip",
        ],
    },
    {
        "id": "M14_turbulent_displace",
        "goal": "Recover the hidden displacement field kernel, seed/evolution semantics, pinning, and field units.",
        "questions": [
            "What do FracAllKernel and Frac1DKernel compute exactly?",
            "How do amount/size/complexity/evolution/random seed feed lookup tables?",
            "Which pinning and out-of-bounds rule is applied?",
        ],
        "binaries": [
            "target/reverse/ae_2026/effects_temporal_noise_distort/TurbulentDisplace.aex",
            "target/reverse/ae_2026/effects_temporal_noise_distort/TurbulentNoise.aex",
        ],
        "predecode_tasks": ["turbulent_displace_aex"],
        "ae_artifacts": [
            "fixtures/ae_probe_pack/turbulent_field/manifest.json",
            "fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/turbulent_field/ae_goldens/metadata/turbulent_vector_measurements.json",
            "fixtures/ae_probe_pack/downloads/ae_probe_round5_85_20260504_011829_tiff_png_outputs_85.zip",
        ],
    },
    {
        "id": "M05_M07_text_glyph_cooltype",
        "goal": "Lock glyph metrics, baseline/line boxes, text blur source space, and collapsed text sharpness substrate.",
        "questions": [
            "Which CoolType/TXT path supplies glyph ids, advances, bboxes, and baselines?",
            "What is AE text animator blur kernel and coordinate space?",
            "When does collapsed precomp defer text rasterization?",
        ],
        "binaries": [
            "target/reverse/ae_2026/text_expression/Basic_Text.aex",
            "target/reverse/ae_2026/text_expression/CoolType.dll",
            "target/reverse/ae_2026/text_expression/AfterFXLib.dll",
        ],
        "predecode_tasks": ["basic_text_aex", "cooltype_glyph_metrics", "cooltype_glyph_metrics_core"],
        "ae_artifacts": [
            "fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf",
            "fixtures/ae_conformance_pack/ae_goldens/metadata/effect_property_dump.json",
        ],
    },
    {
        "id": "M08_M09_expression_host",
        "goal": "Lock property expression host context, layer-local time, vector/scalar coercion, and expression selector variables.",
        "questions": [
            "How do BEE and Scripting convert comp time to layer/expression time?",
            "Where are value/textIndex/textTotal/inPoint/outPoint injected?",
            "Which property value coercions are AE host behavior versus ExtendScript engine behavior?",
        ],
        "binaries": [
            "target/reverse/ae_2026/text_expression/Scripting.aex",
            "target/reverse/ae_2026/text_expression/extendscript.dll",
            "target/reverse/ae_2026/core_composite_alpha/BEE.dll",
        ],
        "predecode_tasks": ["scripting_expression_host", "bee_temporal_scheduler"],
        "ae_artifacts": [
            "fixtures/ae_conformance_pack/manifest.json",
            "fixtures/ae_conformance_pack/ae_goldens/metadata/effect_property_dump.json",
        ],
    },
    {
        "id": "M15_M17_geometry_collapse_motion",
        "goal": "Lock Geometry2 matrix path, transform sampling, motion-blur matrix samples, and collapse/deferred-raster boundary.",
        "questions": [
            "Which Geometry2 params map to GF::Transformation fields?",
            "How does TransformWithMotionBlur choose matrix samples?",
            "Where is the raster barrier for collapsed text/vector layers?",
        ],
        "binaries": [
            "target/reverse/ae_2026/Transform.aex",
            "target/reverse/ae_2026/core_composite_alpha/GPUFoundation.dll",
        ],
        "predecode_tasks": ["geometry_transform_gpufoundation", "geometry_transform_wrapper"],
        "ae_artifacts": [
            "fixtures/ae_conformance_pack/manifest.json",
            "fixtures/ae_conformance_pack/ae_goldens/metadata/tiff_png_summary.json",
        ],
    },
    {
        "id": "M16_temporal_motion_scheduler",
        "goal": "Lock posterize source-time blocking and motion-blur shutter/sample scheduling.",
        "questions": [
            "Which time is bucketed by Posterize Time?",
            "How are shutter angle/phase converted into sample times?",
            "Does expression/effect/sample time use comp or layer local units?",
        ],
        "binaries": [
            "target/reverse/ae_2026/effects_temporal_noise_distort/Posterize_Time.aex",
            "target/reverse/ae_2026/core_composite_alpha/BEE.dll",
        ],
        "predecode_tasks": ["posterize_time", "time_displace_temporal", "bee_temporal_scheduler"],
        "ae_artifacts": ["fixtures/ae_conformance_pack/manifest.json"],
    },
]


def sha256(path: Path) -> str | None:
    if not path.is_file():
        return None
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def rel(path: Path) -> str:
    try:
        return str(path.relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


def latest_predecode_for(task: str) -> dict[str, Any] | None:
    root = REPO_ROOT / "target/reverse/predecoded"
    candidates = sorted(root.glob(f"*/{task}/index.md"))
    if not candidates:
        return None
    index = candidates[-1]
    task_dir = index.parent
    return {
        "path": rel(task_dir),
        "index": rel(index),
        "decompile_count": len(list(task_dir.glob("*/decompile.c"))),
        "disassembly_count": len(list(task_dir.glob("*/disassembly.txt"))),
    }


def count_files(path: Path, suffixes: tuple[str, ...]) -> int:
    if path.is_file():
        return 1 if path.suffix.lower() in suffixes else 0
    if not path.is_dir():
        return 0
    return sum(1 for item in path.rglob("*") if item.is_file() and item.suffix.lower() in suffixes)


def json_summary(path: Path) -> dict[str, Any] | None:
    if not path.is_file() or path.suffix.lower() != ".json":
        return None
    try:
        data = json.loads(path.read_text(encoding="utf-8-sig"))
    except Exception as exc:  # pragma: no cover - reporting script
        return {"valid": False, "error": str(exc)}
    summary: dict[str, Any] = {"valid": True, "type": type(data).__name__}
    if isinstance(data, dict):
        summary["keys"] = sorted(data.keys())
        for key in ("cases", "probe_suites"):
            if isinstance(data.get(key), list):
                summary[f"{key}_count"] = len(data[key])
        if isinstance(data.get("cases"), dict):
            summary["cases_count"] = len(data["cases"])
        if "case_count" in data:
            summary["case_count"] = data["case_count"]
    elif isinstance(data, list):
        summary["count"] = len(data)
    return summary


def build_report() -> dict[str, Any]:
    items = []
    for req in REQUIREMENTS:
        binaries = []
        for path_s in req["binaries"]:
            path = REPO_ROOT / path_s
            binaries.append(
                {
                    "path": path_s,
                    "present": path.is_file(),
                    "bytes": path.stat().st_size if path.is_file() else 0,
                    "sha256": sha256(path),
                }
            )
        predecode = []
        for task in req["predecode_tasks"]:
            found = latest_predecode_for(task)
            predecode.append({"task": task, "present": found is not None, **(found or {})})
        ae_artifacts = []
        for path_s in req["ae_artifacts"]:
            path = REPO_ROOT / path_s
            ae_artifacts.append(
                {
                    "path": path_s,
                    "present": path.exists(),
                    "bytes": path.stat().st_size if path.is_file() else 0,
                    "png_count": count_files(path, (".png",)),
                    "tiff_count": count_files(path, (".tif", ".tiff")),
                    "zip_count": count_files(path, (".zip",)),
                    "json": json_summary(path),
                }
            )
        status = "ready"
        if not all(item["present"] for item in binaries + predecode + ae_artifacts):
            status = "partial"
        if req.get("status_note"):
            status = "partial"
        items.append({**req, "status": status, "binaries": binaries, "predecode": predecode, "ae_artifacts": ae_artifacts})

    return {
        "schema": "ae-native-renderer.reverse-artifact-intake.v1",
        "created_at": dt.datetime.now().isoformat(timespec="seconds"),
        "items": items,
    }


def render_markdown(report: dict[str, Any]) -> str:
    lines = [
        "# Reverse Artifact Intake",
        "",
        f"Generated: `{report['created_at']}`",
        "",
        "This document is the current intake map for AE parity work: what evidence we need, what is already local, and what still blocks formula tuning.",
        "",
        "## Summary",
        "",
        "| Block | Status | Evidence | Missing / Risk |",
        "| --- | --- | --- | --- |",
    ]
    for item in report["items"]:
        present_bins = sum(1 for b in item["binaries"] if b["present"])
        present_pre = sum(1 for p in item["predecode"] if p["present"])
        present_ae = sum(1 for a in item["ae_artifacts"] if a["present"])
        missing = []
        missing += [Path(b["path"]).name for b in item["binaries"] if not b["present"]]
        missing += [p["task"] for p in item["predecode"] if not p["present"]]
        missing += [a["path"] for a in item["ae_artifacts"] if not a["present"]]
        if item.get("status_note"):
            missing.append(item["status_note"])
        lines.append(
            f"| `{item['id']}` | `{item['status']}` | binaries {present_bins}/{len(item['binaries'])}, predecode {present_pre}/{len(item['predecode'])}, AE artifacts {present_ae}/{len(item['ae_artifacts'])} | "
            + (", ".join(missing) if missing else "none")
            + " |"
        )

    lines += ["", "## Blocks", ""]
    for item in report["items"]:
        lines += [
            f"### {item['id']}",
            "",
            item["goal"],
            "",
            "Questions:",
        ]
        lines += [f"- {q}" for q in item["questions"]]
        lines += ["", "Binaries:"]
        for binary in item["binaries"]:
            sha = binary["sha256"][:12] if binary["sha256"] else "missing"
            size = binary["bytes"]
            lines.append(f"- `{binary['path']}`: {'present' if binary['present'] else 'missing'}, bytes={size}, sha256={sha}")
        lines += ["", "Predecode:"]
        for pre in item["predecode"]:
            if pre["present"]:
                lines.append(
                    f"- `{pre['task']}`: `{pre['path']}`, decompile={pre['decompile_count']}, disasm={pre['disassembly_count']}"
                )
            else:
                lines.append(f"- `{pre['task']}`: missing")
        lines += ["", "AE artifacts:"]
        for artifact in item["ae_artifacts"]:
            extra = []
            if artifact["png_count"]:
                extra.append(f"png={artifact['png_count']}")
            if artifact["tiff_count"]:
                extra.append(f"tiff={artifact['tiff_count']}")
            if artifact["zip_count"]:
                extra.append(f"zip={artifact['zip_count']}")
            if artifact["bytes"] and not extra:
                extra.append(f"bytes={artifact['bytes']}")
            if artifact["json"]:
                js = artifact["json"]
                if js.get("valid"):
                    if "cases_count" in js:
                        extra.append(f"cases={js['cases_count']}")
                    if "probe_suites_count" in js:
                        extra.append(f"probe_suites={js['probe_suites_count']}")
                else:
                    extra.append("json=invalid")
            lines.append(f"- `{artifact['path']}`: {'present' if artifact['present'] else 'missing'}" + (f" ({', '.join(extra)})" if extra else ""))
        if item.get("status_note"):
            lines += ["", f"Note: {item['status_note']}"]
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out-dir", type=Path, default=REPO_ROOT / "target/reverse/artifact_intake")
    ap.add_argument("--doc", type=Path, default=REPO_ROOT / "docs/phase_reports/ARTIFACT_INTAKE_20260505.md")
    args = ap.parse_args()

    report = build_report()
    run_dir = args.out_dir / dt.datetime.now().strftime("%Y%m%d_%H%M%S")
    run_dir.mkdir(parents=True, exist_ok=True)
    (run_dir / "artifact_status.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
    markdown = render_markdown(report)
    (run_dir / "artifact_status.md").write_text(markdown, encoding="utf-8")
    args.doc.parent.mkdir(parents=True, exist_ok=True)
    args.doc.write_text(markdown, encoding="utf-8")
    print(json.dumps({"run_dir": rel(run_dir), "doc": rel(args.doc)}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
