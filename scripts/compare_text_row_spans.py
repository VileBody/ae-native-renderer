#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any


def iter_jsonl(path: Path) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if not line.strip():
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(obj, dict) and isinstance(obj.get("payload"), dict):
            events.append(obj["payload"])
        elif isinstance(obj, dict):
            events.append(obj)
    return events


def parse_ae_rows(path: Path, case_id: str) -> dict[str, Any]:
    rows: list[dict[str, Any]] = []
    plane_probes: list[dict[str, Any]] = []
    hook_counts: Counter[str] = Counter()
    plane_probe_hooks: Counter[str] = Counter()
    span_types: Counter[str] = Counter()
    source_pixels: Counter[str] = Counter()
    plane_layouts: Counter[tuple[Any, ...]] = Counter()
    active_role = "unknown"
    pass_index = -1
    last_type2_row: dict[str, Any] | None = None

    for event in iter_jsonl(path):
        if event.get("kind") != "cooltype_hook_enter":
            continue
        hook = str(event.get("hook") or "")
        hook_counts[hook] += 1
        spans = event.get("txt_are_spans") or {}

        if hook == "TXT_ARE_Render_8bpc_fill_3d200":
            active_role = "fill"
            pass_index += 1
        elif hook == "TXT_ARE_Render_8bpc_stroke_3d960":
            active_role = "stroke"
            pass_index += 1

        if hook.startswith("TXT_ARE_PixelWriter8_type2_"):
            probe = {
                "seq": event.get("seq"),
                "hook": hook,
                "regs": spans.get("regs"),
                "memory_samples": spans.get("memory_samples"),
                "candidates": spans.get("candidates") or [],
            }
            plane_probes.append(probe)
            plane_probe_hooks[hook] += 1
            if hook == "TXT_ARE_PixelWriter8_type2_sample_byte_3ba80" and last_type2_row:
                memory_samples = spans.get("memory_samples") or {}
                regs = spans.get("regs") or {}
                rbx_hex = memory_samples.get("rbx_u8_32")
                if isinstance(rbx_hex, str) and len(rbx_hex) >= 2:
                    sample_bytes = last_type2_row.setdefault("_actual_coverage_bytes", [])
                    sample_ptrs = last_type2_row.setdefault("_actual_coverage_ptrs", [])
                    coverage_len = last_type2_row.get("coverage_len")
                    if coverage_len is None or len(sample_bytes) < int(coverage_len):
                        sample_bytes.append(rbx_hex[:2])
                        sample_ptrs.append(regs.get("rbx"))
                        last_type2_row["actual_coverage_sample_hex"] = "".join(sample_bytes)
                        last_type2_row["actual_coverage_sample_count"] = len(sample_bytes)
                        last_type2_row["actual_coverage_first_ptr"] = sample_ptrs[0] if sample_ptrs else None
                        last_type2_row["actual_coverage_last_ptr"] = sample_ptrs[-1] if sample_ptrs else None
            continue

        pixel = spans.get("pixel_writer8_span")
        if not pixel:
            continue

        span_type = pixel.get("span_type_arg3_s32")
        source_pixel = pixel.get("source_pixel8_arg7") or {}
        coverage = pixel.get("coverage_bytes_if_type2") or {}
        row = {
            "schema": "ae-native-renderer.ae-text-row-span.v1",
            "source": "ae_txt_are",
            "case": case_id,
            "seq": event.get("seq"),
            "pass_index": pass_index,
            "pass_role": active_role,
            "span_type": span_type,
            "y": pixel.get("y_arg4_s32"),
            "start_x": pixel.get("start_x_arg5_s32"),
            "end_x": pixel.get("end_x_arg6_s32"),
            "coverage_len": (
                None
                if pixel.get("start_x_arg5_s32") is None or pixel.get("end_x_arg6_s32") is None
                else max(0, int(pixel["end_x_arg6_s32"]) - int(pixel["start_x_arg5_s32"]))
            ),
            "coverage_sample_hex": coverage.get("sample_hex"),
            "coverage_sample_count": coverage.get("sample_count"),
            "coverage_stride": coverage.get("stride"),
            "coverage_stride_source": coverage.get("stride_source"),
            "coverage_plane_size": [coverage.get("width"), coverage.get("height")]
            if coverage
            else None,
            "source_pixel8": source_pixel,
        }
        rows.append(row)
        if span_type == 2:
            last_type2_row = row
        else:
            last_type2_row = None
        span_types[str(span_type)] += 1
        if source_pixel:
            source_pixels[str(source_pixel.get("raw"))] += 1
        if coverage:
            plane_layouts[
                (
                    coverage.get("width"),
                    coverage.get("height"),
                    coverage.get("stride"),
                    coverage.get("stride_source"),
                )
            ] += 1

    for row in rows:
        row.pop("_actual_coverage_bytes", None)
        row.pop("_actual_coverage_ptrs", None)

    return {
        "schema": "ae-native-renderer.text-row-span-parse.v1",
        "case": case_id,
        "source_path": str(path),
        "rows": rows,
        "plane_probes": plane_probes,
        "summary": {
            "row_count": len(rows),
            "type2_row_count": sum(1 for row in rows if row.get("span_type") == 2),
            "hook_counts": dict(hook_counts),
            "span_type_counts": dict(span_types),
            "source_pixel_counts": dict(source_pixels),
            "plane_layout_counts": [
                {"layout": list(key), "count": count}
                for key, count in plane_layouts.most_common()
            ],
            "plane_probe_count": len(plane_probes),
            "plane_probe_hooks": dict(plane_probe_hooks),
        },
    }


def parse_native_rows(path: Path, case_id: str) -> dict[str, Any]:
    rows: list[dict[str, Any]] = []
    per_glyph: Counter[str] = Counter()
    for event in iter_jsonl(path):
        if event.get("event") != "text.layout":
            continue
        frame = event.get("frame")
        record = event.get("record") or {}
        draw_char = record.get("draw_char") or {}
        for glyph in draw_char.get("draw_chars") or []:
            glyph_id = glyph.get("glyph_id")
            glyph_run_index = glyph.get("glyph_run_index")
            for row in glyph.get("coverage_rows") or []:
                out = {
                    "schema": "ae-native-renderer.native-text-row-span.v1",
                    "source": "native_text_engine",
                    "case": case_id,
                    "frame": frame,
                    "glyph_run_index": glyph_run_index,
                    "glyph_id": glyph_id,
                    "y": row.get("y"),
                    "start_x": row.get("start_x"),
                    "end_x": row.get("end_x"),
                    "coverage_len": row.get("coverage_len"),
                    "coverage_hash_fnv1a64": row.get("coverage_hash_fnv1a64"),
                    "coverage_sample_hex": row.get("coverage_sample_hex"),
                    "coverage_hex": row.get("coverage_hex"),
                }
                rows.append(out)
                per_glyph[str(glyph_id)] += 1

    by_key: defaultdict[tuple[Any, Any, Any], list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        by_key[(row.get("y"), row.get("start_x"), row.get("end_x"))].append(row)

    return {
        "schema": "ae-native-renderer.native-text-row-span-parse.v1",
        "case": case_id,
        "source_path": str(path),
        "rows": rows,
        "summary": {
            "row_count": len(rows),
            "glyph_row_counts": dict(per_glyph),
            "unique_y_start_end": len(by_key),
        },
    }


def compare_rows(ae: dict[str, Any], native: dict[str, Any]) -> dict[str, Any]:
    ae_type2 = [row for row in ae.get("rows", []) if row.get("span_type") == 2]
    native_rows = native.get("rows", [])
    ae_keys = Counter((row.get("y"), row.get("start_x"), row.get("end_x")) for row in ae_type2)
    native_keys = Counter((row.get("y"), row.get("start_x"), row.get("end_x")) for row in native_rows)
    native_by_key: defaultdict[tuple[Any, Any, Any], list[dict[str, Any]]] = defaultdict(list)
    for row in native_rows:
        native_by_key[(row.get("y"), row.get("start_x"), row.get("end_x"))].append(row)
    common = ae_keys & native_keys
    ae_only = ae_keys - native_keys
    native_only = native_keys - ae_keys
    sample_matches = 0
    sample_mismatches = 0
    sample_missing = 0
    sample_mismatch_examples: list[dict[str, Any]] = []
    for row in ae_type2:
        key = (row.get("y"), row.get("start_x"), row.get("end_x"))
        candidates = native_by_key.get(key) or []
        if not candidates:
            continue
        ae_hex = row.get("actual_coverage_sample_hex") or row.get("coverage_sample_hex")
        native_hex = candidates[0].get("coverage_hex") or candidates[0].get("coverage_sample_hex")
        if not ae_hex or not native_hex:
            sample_missing += 1
            continue
        n = min(len(str(ae_hex)), len(str(native_hex)))
        if str(ae_hex)[:n] == str(native_hex)[:n]:
            sample_matches += 1
        else:
            sample_mismatches += 1
            if len(sample_mismatch_examples) < 12:
                sample_mismatch_examples.append({
                    "y": key[0],
                    "start_x": key[1],
                    "end_x": key[2],
                    "ae_hex": str(ae_hex)[:96],
                    "native_hex": str(native_hex)[:96],
                })
    return {
        "schema": "ae-native-renderer.text-row-span-compare.v1",
        "ae_type2_rows": len(ae_type2),
        "native_rows": len(native_rows),
        "common_y_start_end": sum(common.values()),
        "ae_only_y_start_end": sum(ae_only.values()),
        "native_only_y_start_end": sum(native_only.values()),
        "coverage_sample_matches": sample_matches,
        "coverage_sample_mismatches": sample_mismatches,
        "coverage_sample_missing": sample_missing,
        "coverage_sample_mismatch_examples": sample_mismatch_examples,
        "ae_only_examples": [
            {"y": k[0], "start_x": k[1], "end_x": k[2], "count": v}
            for k, v in ae_only.most_common(12)
        ],
        "native_only_examples": [
            {"y": k[0], "start_x": k[1], "end_x": k[2], "count": v}
            for k, v in native_only.most_common(12)
        ],
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--ae-jsonl", default="")
    ap.add_argument("--native-text-telemetry", default="")
    ap.add_argument("--case", required=True)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    result: dict[str, Any] = {"case": args.case}
    if args.ae_jsonl:
        result["ae"] = parse_ae_rows(Path(args.ae_jsonl), args.case)
    if args.native_text_telemetry:
        result["native"] = parse_native_rows(Path(args.native_text_telemetry), args.case)
    if "ae" in result and "native" in result:
        result["comparison"] = compare_rows(result["ae"], result["native"])

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps({
        "out": str(out),
        "ae_rows": result.get("ae", {}).get("summary", {}).get("row_count"),
        "native_rows": result.get("native", {}).get("summary", {}).get("row_count"),
        "plane_probe_count": result.get("ae", {}).get("summary", {}).get("plane_probe_count"),
    }, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
