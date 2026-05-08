#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any


FNV1A64_OFFSET = 0xCBF29CE484222325
FNV1A64_PRIME = 0x100000001B3


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
                "actual_span_count": spans.get("actual_span_count"),
                "actual_span_count_source": spans.get("actual_span_count_source"),
                "actual_span_count_rax": spans.get("actual_span_count_rax"),
                "actual_span_count_rsi": spans.get("actual_span_count_rsi"),
                "actual_span_rbx_hex": spans.get("actual_span_rbx_hex"),
                "actual_span_first_ptr": spans.get("actual_span_first_ptr"),
                "actual_span_last_ptr": spans.get("actual_span_last_ptr"),
                "candidates": spans.get("candidates") or [],
            }
            plane_probes.append(probe)
            plane_probe_hooks[hook] += 1
            if hook in {
                "TXT_ARE_PixelWriter8_type2_stride_add_3ba5b",
                "TXT_ARE_PixelWriter8_type2_span_count_3ba71",
                "TXT_ARE_PixelWriter8_type2_span_ready_3ba74",
            } and last_type2_row:
                actual_hex = spans.get("actual_span_rbx_hex")
                actual_count = (
                    spans.get("actual_span_count")
                    if spans.get("actual_span_count") is not None
                    else spans.get("actual_span_count_rsi")
                )
                if actual_count is None:
                    actual_count = spans.get("actual_span_count_rax")
                row_coverage_len = last_type2_row.get("coverage_len")
                sample_count = actual_count
                if row_coverage_len is not None:
                    sample_count = int(row_coverage_len)
                if not isinstance(actual_hex, str) and hook == "TXT_ARE_PixelWriter8_type2_stride_add_3ba5b":
                    memory_samples = spans.get("memory_samples") or {}
                    rbx_hex = memory_samples.get("rbx_u8_32")
                    if isinstance(rbx_hex, str):
                        if sample_count is not None:
                            actual_hex = rbx_hex[: max(0, int(sample_count)) * 2]
                        else:
                            actual_hex = rbx_hex
                elif isinstance(actual_hex, str) and sample_count is not None:
                    actual_hex = actual_hex[: max(0, int(sample_count)) * 2]
                if isinstance(actual_hex, str):
                    last_type2_row["actual_coverage_sample_hex"] = actual_hex
                    last_type2_row["actual_coverage_sample_count"] = (
                        sample_count if sample_count is not None else len(actual_hex) // 2
                    )
                    last_type2_row["actual_coverage_first_ptr"] = spans.get("actual_span_first_ptr")
                    last_type2_row["actual_coverage_last_ptr"] = spans.get("actual_span_last_ptr")
            elif hook == "TXT_ARE_PixelWriter8_type2_sample_byte_3ba80" and last_type2_row:
                if last_type2_row.get("actual_coverage_sample_hex"):
                    continue
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
    ink_rows = build_ae_ink_rows(rows)

    return {
        "schema": "ae-native-renderer.text-row-span-parse.v1",
        "case": case_id,
        "source_path": str(path),
        "rows": rows,
        "ink_rows": ink_rows,
        "plane_probes": plane_probes,
        "summary": {
            "row_count": len(rows),
            "type2_row_count": sum(1 for row in rows if row.get("span_type") == 2),
            "ink_row_count": len(ink_rows),
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


def build_ae_ink_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    ink_rows: list[dict[str, Any]] = []
    current: dict[str, Any] | None = None
    for row in rows:
        span_type = row.get("span_type")
        if span_type not in {1, 2}:
            if current is not None:
                ink_rows.append(current)
                current = None
            continue
        y = row.get("y")
        start_x = row.get("start_x")
        end_x = row.get("end_x")
        if y is None or start_x is None or end_x is None:
            continue
        part = {
            "span_type": span_type,
            "start_x": start_x,
            "end_x": end_x,
            "coverage_hex": row.get("actual_coverage_sample_hex")
            or row.get("coverage_sample_hex")
            or ("ff" * max(0, int(end_x) - int(start_x)) if span_type == 1 else None),
        }
        if current is not None and current.get("y") == y and current.get("end_x") == start_x:
            current["end_x"] = end_x
            current["coverage_len"] = int(current["end_x"]) - int(current["start_x"])
            current.setdefault("parts", []).append(part)
        else:
            if current is not None:
                ink_rows.append(current)
            current = {
                "schema": "ae-native-renderer.ae-text-ink-row-span.v1",
                "source": "ae_txt_are",
                "case": row.get("case"),
                "y": y,
                "start_x": start_x,
                "end_x": end_x,
                "coverage_len": int(end_x) - int(start_x),
                "parts": [part],
            }
    if current is not None:
        ink_rows.append(current)
    case_id = str(rows[0].get("case") if rows else "")
    return enrich_ink_rows(ink_rows, case_id, "ae_txt_are")


def fnv1a64(bytes_value: bytes) -> str:
    value = FNV1A64_OFFSET
    for byte in bytes_value:
        value ^= byte
        value = (value * FNV1A64_PRIME) & 0xFFFFFFFFFFFFFFFF
    return f"{value:016x}"


def hex_to_bytes(hex_value: Any) -> bytes | None:
    if not isinstance(hex_value, str) or len(hex_value) == 0:
        return None
    text = hex_value.strip()
    if len(text) % 2 != 0:
        return None
    try:
        return bytes.fromhex(text)
    except ValueError:
        return None


def coverage_hex(row: dict[str, Any]) -> str | None:
    value = (
        row.get("actual_coverage_sample_hex")
        or row.get("coverage_hex")
        or row.get("coverage_sample_hex")
    )
    return value if isinstance(value, str) and value else None


def coverage_hash(row: dict[str, Any]) -> str | None:
    explicit = row.get("coverage_hash_fnv1a64")
    if isinstance(explicit, str) and explicit:
        return explicit
    bytes_value = hex_to_bytes(coverage_hex(row))
    return fnv1a64(bytes_value) if bytes_value is not None else None


def reconstruct_coverage_hex_from_parts(parts: Any) -> str | None:
    if not isinstance(parts, list) or not parts:
        return None
    chunks: list[str] = []
    expected_start: int | None = None
    for part in parts:
        if not isinstance(part, dict):
            return None
        try:
            start = int(part["start_x"])
            end = int(part["end_x"])
        except (KeyError, TypeError, ValueError):
            return None
        if end < start:
            return None
        if expected_start is not None and start != expected_start:
            return None
        length = end - start
        span_type = part.get("span_type")
        if span_type == 1:
            chunks.append("ff" * length)
        elif span_type == 2:
            value = coverage_hex(part)
            if not isinstance(value, str) or len(value) < length * 2:
                return None
            chunks.append(value[: length * 2])
        else:
            return None
        expected_start = end
    return "".join(chunks)


def enrich_ink_rows(rows: list[dict[str, Any]], case_id: str, source: str) -> list[dict[str, Any]]:
    enriched: list[dict[str, Any]] = []
    for row in rows:
        out = dict(row)
        out.setdefault("schema", "ae-native-renderer.ae-text-ink-row-span.v1")
        out.setdefault("source", source)
        out["case"] = case_id
        if "coverage_len" not in out and out.get("start_x") is not None and out.get("end_x") is not None:
            out["coverage_len"] = int(out["end_x"]) - int(out["start_x"])
        reconstructed = coverage_hex(out) or reconstruct_coverage_hex_from_parts(out.get("parts"))
        if reconstructed:
            out["coverage_hex"] = reconstructed
            out["coverage_sample_hex"] = reconstructed
            out["coverage_hash_fnv1a64"] = fnv1a64(bytes.fromhex(reconstructed))
        enriched.append(out)
    return enriched


def parse_are_row_getter_analysis(path: Path, case_id: str) -> dict[str, Any]:
    raw = json.loads(path.read_text(encoding="utf-8"))
    rows: list[dict[str, Any]] = []
    span_types: Counter[str] = Counter()
    for row in raw.get("row_getter_rows") or []:
        if not isinstance(row, dict):
            continue
        out = dict(row)
        out.setdefault("schema", "ae-native-renderer.are-row-getter-row.v1")
        out["source"] = "ARE_row_getter_8230"
        out["case"] = case_id
        if "coverage_sample_hex" not in out and isinstance(out.get("coverage_hex"), str):
            out["coverage_sample_hex"] = out["coverage_hex"]
        rows.append(out)
        span_types[str(out.get("span_type"))] += 1

    ink_rows = enrich_ink_rows(
        [
            dict(row)
            for row in raw.get("row_getter_ink_rows") or []
            if isinstance(row, dict)
        ],
        case_id,
        "ARE_row_getter_8230",
    )
    if not ink_rows:
        ink_rows = build_ae_ink_rows(rows)

    hook_counts = {
        str(item.get("key")): item.get("count")
        for item in raw.get("hook_counts") or []
        if isinstance(item, dict)
    }
    return {
        "schema": "ae-native-renderer.text-row-span-parse.v1",
        "case": case_id,
        "source_path": str(path),
        "source_kind": "are_row_getter_analysis",
        "rows": rows,
        "ink_rows": ink_rows,
        "summary": {
            "row_count": len(rows),
            "type2_row_count": sum(1 for row in rows if row.get("span_type") == 2),
            "ink_row_count": len(ink_rows),
            "hook_counts": hook_counts,
            "span_type_counts": dict(span_types),
            "trace_path": raw.get("trace_path"),
        },
    }


def first_byte_diff(a: bytes, b: bytes) -> dict[str, Any] | None:
    for index, (av, bv) in enumerate(zip(a, b)):
        if av != bv:
            return {"index": index, "ae": av, "native": bv, "delta": av - bv}
    if len(a) != len(b):
        return {"index": min(len(a), len(b)), "ae_len": len(a), "native_len": len(b)}
    return None


def adapt_native_rows_to_ae_like(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    adapted = []
    for row in rows:
        adapted.append({
            "schema": "ae-native-renderer.native-text-row-span-ae-like.v1",
            "source": "native_text_engine",
            "case": row.get("case"),
            "frame": row.get("frame"),
            "pass_index": 0,
            "pass_role": "fill",
            "span_type": 2,
            "y": row.get("y"),
            "start_x": row.get("start_x"),
            "end_x": row.get("end_x"),
            "coverage_len": row.get("coverage_len"),
            "coverage_sample_hex": row.get("coverage_sample_hex"),
            "coverage_hex": row.get("coverage_hex"),
            "coverage_hash_fnv1a64": row.get("coverage_hash_fnv1a64"),
            "glyph_run_index": row.get("glyph_run_index"),
            "glyph_id": row.get("glyph_id"),
        })
    return adapted


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
        "ae_like_rows": adapt_native_rows_to_ae_like(rows),
        "summary": {
            "row_count": len(rows),
            "glyph_row_counts": dict(per_glyph),
            "unique_y_start_end": len(by_key),
            "rows_with_coverage_hex": sum(1 for row in rows if row.get("coverage_hex")),
            "rows_with_coverage_hash": sum(1 for row in rows if row.get("coverage_hash_fnv1a64")),
        },
    }


def compare_coverage_bytes(
    ae_type2: list[dict[str, Any]],
    native_by_key: dict[tuple[Any, Any, Any], list[dict[str, Any]]],
    align: str = "absolute",
) -> dict[str, Any]:
    compared = 0
    exact = 0
    prefix_exact = 0
    hash_exact = 0
    missing = 0
    mismatch_examples: list[dict[str, Any]] = []
    ae_unique_values: Counter[int] = Counter()
    native_unique_values: Counter[int] = Counter()

    for row in ae_type2:
        key = (row.get("y"), row.get("start_x"), row.get("end_x"))
        candidates = native_by_key.get(key) or []
        if not candidates:
            continue
        native = candidates[0]
        ae_hex = coverage_hex(row)
        native_hex = coverage_hex(native)
        ae_bytes = hex_to_bytes(ae_hex)
        native_bytes = hex_to_bytes(native_hex)
        if ae_bytes is None or native_bytes is None:
            missing += 1
            continue

        compared += 1
        ae_unique_values.update(ae_bytes)
        native_unique_values.update(native_bytes)
        min_len = min(len(ae_bytes), len(native_bytes))
        is_prefix_exact = ae_bytes[:min_len] == native_bytes[:min_len]
        is_exact = is_prefix_exact and len(ae_bytes) == len(native_bytes)
        if is_prefix_exact:
            prefix_exact += 1
        if is_exact:
            exact += 1
        if coverage_hash(row) is not None and coverage_hash(row) == coverage_hash(native):
            hash_exact += 1
        if not is_exact and len(mismatch_examples) < 12:
            mismatch_examples.append({
                "y": key[0],
                "start_x": key[1],
                "end_x": key[2],
                "ae_hex": (ae_hex or "")[:96],
                "native_hex": (native_hex or "")[:96],
                "ae_len": len(ae_bytes),
                "native_len": len(native_bytes),
                "first_diff": first_byte_diff(ae_bytes, native_bytes),
            })

    ratio = exact / compared if compared else None
    prefix_ratio = prefix_exact / compared if compared else None
    hash_ratio = hash_exact / compared if compared else None
    return {
        "schema": "ae-native-renderer.text-row-coverage-byte-compare.v1",
        "align": align,
        "compared_rows": compared,
        "exact_rows": exact,
        "prefix_exact_rows": prefix_exact,
        "hash_exact_rows": hash_exact,
        "missing_rows": missing,
        "exact_ratio": ratio,
        "prefix_exact_ratio": prefix_ratio,
        "hash_exact_ratio": hash_ratio,
        "ae_unique_byte_values": len(ae_unique_values),
        "native_unique_byte_values": len(native_unique_values),
        "ae_top_byte_values": [
            {"value": value, "count": count}
            for value, count in ae_unique_values.most_common(16)
        ],
        "native_top_byte_values": [
            {"value": value, "count": count}
            for value, count in native_unique_values.most_common(16)
        ],
        "mismatch_examples": mismatch_examples,
    }


def compare_coverage_bytes_normalized(
    ae_type2: list[dict[str, Any]],
    native_rows: list[dict[str, Any]],
) -> dict[str, Any]:
    def usable(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
        return [
            row
            for row in rows
            if row.get("y") is not None
            and row.get("start_x") is not None
            and row.get("end_x") is not None
        ]

    ae_rows = usable(ae_type2)
    native_usable = usable(native_rows)
    if not ae_rows or not native_usable:
        return compare_coverage_bytes(ae_type2, {}, align="normalized")

    ae_min_y = min(int(row["y"]) for row in ae_rows)
    ae_min_x = min(int(row["start_x"]) for row in ae_rows)
    native_min_y = min(int(row["y"]) for row in native_usable)
    native_min_x = min(int(row["start_x"]) for row in native_usable)

    def key(row: dict[str, Any], min_y: int, min_x: int) -> tuple[int, int, int]:
        return (
            int(row["y"]) - min_y,
            int(row["start_x"]) - min_x,
            int(row["end_x"]) - min_x,
        )

    native_by_key: defaultdict[tuple[Any, Any, Any], list[dict[str, Any]]] = defaultdict(list)
    for row in native_usable:
        native_by_key[key(row, native_min_y, native_min_x)].append(row)

    ae_normalized = []
    for row in ae_rows:
        normalized = dict(row)
        norm_y, norm_start, norm_end = key(row, ae_min_y, ae_min_x)
        normalized["y"] = norm_y
        normalized["start_x"] = norm_start
        normalized["end_x"] = norm_end
        ae_normalized.append(normalized)

    result = compare_coverage_bytes(ae_normalized, native_by_key, align="normalized")
    result["origins"] = {
        "ae": {"y": ae_min_y, "x": ae_min_x},
        "native": {"y": native_min_y, "x": native_min_x},
    }
    return result


def acceptance_summary(
    ae_type2_count: int,
    native_count: int,
    common_count: int,
    normalized_ink: dict[str, Any],
    byte_compare: dict[str, Any],
) -> dict[str, Any]:
    shape_denominator = max(ae_type2_count, native_count, 1)
    absolute_type2_shape_ratio = common_count / shape_denominator
    ink_ae = (normalized_ink.get("ae") or {}).get("row_count") or 0
    ink_native = (normalized_ink.get("native") or {}).get("row_count") or 0
    ink_common = normalized_ink.get("common_y_start_end") or 0
    ink_ratio = ink_common / max(ink_ae, ink_native, 1)
    byte_ratio = byte_compare.get("exact_ratio")
    if byte_compare.get("compared_rows", 0) == 0:
        status = "instrumentation-incomplete"
    elif ink_ratio >= 0.99 and (byte_ratio or 0.0) >= 0.99:
        status = "accepted"
    elif absolute_type2_shape_ratio >= 0.90 or ink_ratio >= 0.90:
        status = "formula-tuning"
    else:
        status = "substrate-mismatch"
    return {
        "schema": "ae-native-renderer.text-row-span-acceptance.v1",
        "status": status,
        "shape_exact_ratio": absolute_type2_shape_ratio,
        "normalized_ink_shape_ratio": ink_ratio,
        "coverage_byte_exact_ratio": byte_ratio,
        "criteria": {
            "accepted": "normalized_ink_shape_ratio>=0.99 and coverage_byte_exact_ratio>=0.99",
            "formula_tuning": "shape is mostly aligned but coverage bytes still diverge",
            "instrumentation_incomplete": "no comparable coverage bytes were captured",
        },
    }


def compare_rows(ae: dict[str, Any], native: dict[str, Any]) -> dict[str, Any]:
    ae_type2 = [row for row in ae.get("rows", []) if row.get("span_type") == 2]
    ae_ink_rows = ae.get("ink_rows") or []
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
    normalized = compare_normalized_rows(ae_type2, native_rows)
    normalized_ink = compare_normalized_rows(ae_ink_rows, native_rows)
    byte_compare_absolute = compare_coverage_bytes(ae_type2, native_by_key)
    byte_compare = compare_coverage_bytes_normalized(ae_type2, native_rows)
    byte_compare_ink = compare_coverage_bytes_normalized(ae_ink_rows, native_rows)
    acceptance = acceptance_summary(
        len(ae_type2),
        len(native_rows),
        sum(common.values()),
        normalized_ink,
        byte_compare_ink if byte_compare_ink.get("compared_rows", 0) else byte_compare,
    )
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
        "coverage_byte_compare": byte_compare,
        "coverage_byte_compare_ink": byte_compare_ink,
        "coverage_byte_compare_absolute": byte_compare_absolute,
        "acceptance": acceptance,
        "coverage_sample_mismatch_examples": sample_mismatch_examples,
        "normalized_shape": normalized,
        "normalized_ink_shape": normalized_ink,
        "ae_only_examples": [
            {"y": k[0], "start_x": k[1], "end_x": k[2], "count": v}
            for k, v in ae_only.most_common(12)
        ],
        "native_only_examples": [
            {"y": k[0], "start_x": k[1], "end_x": k[2], "count": v}
            for k, v in native_only.most_common(12)
        ],
    }


def compare_normalized_rows(ae_rows: list[dict[str, Any]], native_rows: list[dict[str, Any]]) -> dict[str, Any]:
    def normalized_keys(rows: list[dict[str, Any]]) -> tuple[Counter[tuple[int, int, int]], dict[str, Any]]:
        usable = [
            row
            for row in rows
            if row.get("y") is not None
            and row.get("start_x") is not None
            and row.get("end_x") is not None
        ]
        if not usable:
            return Counter(), {"row_count": 0}
        min_y = min(int(row["y"]) for row in usable)
        min_x = min(int(row["start_x"]) for row in usable)
        max_y = max(int(row["y"]) for row in usable)
        max_x = max(int(row["end_x"]) for row in usable)
        keys = Counter(
            (
                int(row["y"]) - min_y,
                int(row["start_x"]) - min_x,
                int(row["end_x"]) - min_x,
            )
            for row in usable
        )
        return keys, {
            "row_count": len(usable),
            "origin": {"y": min_y, "x": min_x},
            "extent": {"height": max_y - min_y + 1, "width": max_x - min_x},
            "unique_y_start_end": len(keys),
        }

    ae_keys, ae_stats = normalized_keys(ae_rows)
    native_keys, native_stats = normalized_keys(native_rows)
    common = ae_keys & native_keys
    ae_only = ae_keys - native_keys
    native_only = native_keys - ae_keys
    return {
        "schema": "ae-native-renderer.text-row-span-normalized-shape.v1",
        "ae": ae_stats,
        "native": native_stats,
        "common_y_start_end": sum(common.values()),
        "ae_only_y_start_end": sum(ae_only.values()),
        "native_only_y_start_end": sum(native_only.values()),
        "ae_only_examples": [
            {"norm_y": k[0], "start_x": k[1], "end_x": k[2], "count": v}
            for k, v in ae_only.most_common(12)
        ],
        "native_only_examples": [
            {"norm_y": k[0], "start_x": k[1], "end_x": k[2], "count": v}
            for k, v in native_only.most_common(12)
        ],
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--ae-jsonl", default="")
    ap.add_argument("--ae-row-getter-analysis", default="")
    ap.add_argument("--native-text-telemetry", default="")
    ap.add_argument("--case", required=True)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    result: dict[str, Any] = {"case": args.case}
    if args.ae_jsonl and args.ae_row_getter_analysis:
        raise SystemExit("--ae-jsonl and --ae-row-getter-analysis are mutually exclusive")
    if args.ae_jsonl:
        result["ae"] = parse_ae_rows(Path(args.ae_jsonl), args.case)
    if args.ae_row_getter_analysis:
        result["ae"] = parse_are_row_getter_analysis(Path(args.ae_row_getter_analysis), args.case)
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
        "ae_ink_rows": result.get("ae", {}).get("summary", {}).get("ink_row_count"),
        "native_rows": result.get("native", {}).get("summary", {}).get("row_count"),
        "plane_probe_count": result.get("ae", {}).get("summary", {}).get("plane_probe_count"),
    }, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
