#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any


def payloads(path: Path):
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue
            if obj.get("type") == "send" and isinstance(obj.get("payload"), dict):
                obj = obj["payload"]
            if isinstance(obj, dict):
                yield obj


def dig(obj: dict[str, Any], *keys: str) -> Any:
    cur: Any = obj
    for key in keys:
        if not isinstance(cur, dict):
            return None
        cur = cur.get(key)
    return cur


def short_hex(value: Any, chars: int = 64) -> str | None:
    if not isinstance(value, str):
        return None
    return value[:chars]


def counter_json(counter: Counter[Any], limit: int | None = None) -> list[dict[str, Any]]:
    return [
        {"key": list(key) if isinstance(key, tuple) else key, "count": count}
        for key, count in counter.most_common(limit)
    ]


def compact_words(words: Any, offsets: set[int]) -> dict[str, Any]:
    out: dict[str, Any] = {}
    if not isinstance(words, list):
        return out
    for word in words:
        if not isinstance(word, dict):
            continue
        off = word.get("offset")
        if off not in offsets:
            continue
        out[f"0x{int(off):x}"] = {
            "ptr": word.get("ptr"),
            "s32_0": word.get("s32_0"),
            "s32_4": word.get("s32_4"),
            "f32_0": word.get("f32_0"),
            "f32_4": word.get("f32_4"),
        }
    return out


def compact_edge(edge: Any) -> dict[str, Any] | None:
    if not isinstance(edge, dict):
        return None
    return {
        "ptr": edge.get("ptr"),
        "x0": edge.get("x0_0x18_f32"),
        "y0": edge.get("y0_0x1c_f32"),
        "x1": edge.get("x1_0x20_f32"),
        "y1": edge.get("y1_0x24_f32"),
        "projected_min": edge.get("projected_min_0x28_f32"),
        "projected_max": edge.get("projected_max_0x2c_f32"),
        "dirty": edge.get("dirty_0x30_u8"),
        "winding_flag": edge.get("winding_flag_0x33_s8"),
        "slope_or_sentinel": edge.get("slope_or_sentinel_0x34_f32"),
        "word0_0x0": compact_words(edge.get("words"), {0, 8, 16}),
    }


def compact_command_queue(command_queue: Any) -> dict[str, Any] | None:
    if not isinstance(command_queue, dict):
        return None
    ctx = command_queue.get("command_ctx_arg1")
    row_ctx = command_queue.get("row_add_ctx_arg1")
    out: dict[str, Any] = {
        "phase": command_queue.get("phase"),
        "hook": command_queue.get("hook"),
        "regs": command_queue.get("regs"),
        "edge_arg2": compact_edge(command_queue.get("edge_arg2")),
        "edge_arg2_peer_0x10": compact_edge(command_queue.get("edge_arg2_peer_0x10")),
        "vector_arg1": command_queue.get("vector_arg1"),
        "edge_slot_arg2": command_queue.get("edge_slot_arg2"),
    }
    if isinstance(ctx, dict):
        out["command_ctx"] = {
            "ptr": ctx.get("ptr"),
            "first_0xe0": ctx.get("first_0xe0"),
            "last_0xe8": ctx.get("last_0xe8"),
            "first_edge": compact_edge(ctx.get("first_edge")),
            "last_edge": compact_edge(ctx.get("last_edge")),
            "queue_0x90": ctx.get("queue_0x90"),
            "current_point_0xd4": ctx.get("current_point_0xd4"),
            "previous_point_0x10c": ctx.get("previous_point_0x10c"),
            "flags": ctx.get("flags"),
        }
    if isinstance(row_ctx, dict):
        out["row_add_ctx"] = row_ctx
    edge_candidates = command_queue.get("edge_candidates")
    if isinstance(edge_candidates, dict):
        out["edge_candidates"] = {
            key: compact_edge(value) for key, value in edge_candidates.items()
        }
    return out


def compact_bucket(bucket: Any) -> dict[str, Any] | None:
    if not isinstance(bucket, dict):
        return None
    sample = bucket.get("sample")
    if not isinstance(sample, dict):
        return {"index": bucket.get("index"), "ptr": bucket.get("ptr")}
    return {
        "index": bucket.get("index"),
        "ptr": bucket.get("ptr"),
        "active_edge_ptr_0x38": sample.get("active_edge_ptr_0x38"),
        "active_edge": compact_edge(sample.get("active_edge")),
        "words": compact_words(sample.get("words"), {0, 8, 16, 24, 32, 40, 48, 56, 64, 72}),
    }


def sampler_summary(sampler: Any) -> dict[str, Any] | None:
    if not isinstance(sampler, dict):
        return None
    buckets = []
    active_count = 0
    for bucket in sampler.get("buckets") or []:
        compact = compact_bucket(bucket)
        if compact is None:
            continue
        active = compact.get("active_edge")
        if isinstance(active, dict) and active.get("ptr"):
            active_count += 1
        if len(buckets) < 16:
            buckets.append(compact)
    return {
        "ptr": sampler.get("ptr"),
        "plane_base_0x48": sampler.get("plane_base_0x48"),
        "row_origin_or_bias_0x88_s32": sampler.get("row_origin_or_bias_0x88_s32"),
        "row_min_0x158_s32": sampler.get("row_min_0x158_s32"),
        "row_max_0x160_s32": sampler.get("row_max_0x160_s32"),
        "current_row_0x188_s32": sampler.get("current_row_0x188_s32"),
        "in_range_0x18c_u8": sampler.get("in_range_0x18c_u8"),
        "row_state_0x190_s64": sampler.get("row_state_0x190_s64"),
        "next_eval_row_0x198_s32": sampler.get("next_eval_row_0x198_s32"),
        "eval_valid_0x260_u8": sampler.get("eval_valid_0x260_u8"),
        "coverage_accum_0x264_s32": sampler.get("coverage_accum_0x264_s32"),
        "next_event_fixed_0x268_s32": sampler.get("next_event_fixed_0x268_s32"),
        "prepared_0x26c_u8": sampler.get("prepared_0x26c_u8"),
        "plane_prefix_64": short_hex(sampler.get("plane_prefix_64"), 128),
        "active_bucket_count": active_count,
        "buckets": buckets,
    }


def row_getter_summary(row_getter: Any) -> dict[str, Any] | None:
    if not isinstance(row_getter, dict):
        return None
    node = row_getter.get("selected_row_head_node")
    node_out = None
    if isinstance(node, dict):
        node_out = {
            "ptr": node.get("ptr"),
            "x_start_0x00_s32": node.get("x_start_0x00_s32"),
            "len_0x04_s32": node.get("len_0x04_s32"),
            "bytes_0x08": node.get("bytes_0x08"),
            "next_0x10": node.get("next_0x10"),
            "state_0x18_s32": node.get("state_0x18_s32"),
            "bytes_sample": short_hex(node.get("bytes_sample"), 128),
        }
    span_cells = []
    for cell in row_getter.get("span_buffer_cells") or []:
        if not isinstance(cell, dict):
            continue
        if len(span_cells) >= 16:
            break
        span_cells.append(
            {
                "index": cell.get("index"),
                "span_type_s32": cell.get("span_type_s32"),
                "end_x_s32": cell.get("end_x_s32"),
                "raw": cell.get("raw"),
            }
        )
    return {
        "ptr": row_getter.get("ptr"),
        "y_arg": row_getter.get("y_arg"),
        "y_min_0x8c": row_getter.get("y_min_0x8c"),
        "y_max_0x94": row_getter.get("y_max_0x94"),
        "cached_y_0xc8": row_getter.get("cached_y_0xc8"),
        "clip_x0_0xd8": row_getter.get("clip_x0_0xd8"),
        "clip_x1_0xe0": row_getter.get("clip_x1_0xe0"),
        "scratch_row_bytes_0xe8": row_getter.get("scratch_row_bytes_0xe8"),
        "span_buffer_0x110": row_getter.get("span_buffer_0x110"),
        "row_table_0x150": row_getter.get("row_table_0x150"),
        "selected_row_head": row_getter.get("selected_row_head"),
        "selected_row_head_node": node_out,
        "scratch_prefix_64": short_hex(row_getter.get("scratch_prefix_64"), 128),
        "scratch_prefix_192": short_hex(row_getter.get("scratch_prefix_192"), 384),
        "span_buffer_cells": span_cells,
    }


def compact_event(event: dict[str, Any], phase: str, body: dict[str, Any]) -> dict[str, Any]:
    return {
        "seq": event.get("seq"),
        "phase": phase,
        "hook": event.get("hook"),
        "arg1": event.get("arg1"),
        "arg2": event.get("arg2"),
        "arg3": event.get("arg3"),
        **body,
    }


def compact_active_edge_hook(active_edge: Any) -> dict[str, Any] | None:
    if not isinstance(active_edge, dict):
        return None
    out = {
        "phase": active_edge.get("phase"),
        "hook": active_edge.get("hook"),
        "regs": active_edge.get("regs"),
        "rcx_edge_candidate": compact_edge(active_edge.get("rcx_edge_candidate")),
        "rdx_edge_candidate": compact_edge(active_edge.get("rdx_edge_candidate")),
        "r8_edge_candidate": compact_edge(active_edge.get("r8_edge_candidate")),
        "r9_edge_candidate": compact_edge(active_edge.get("r9_edge_candidate")),
    }
    for key in [
        "rcx_active_list_candidates",
        "rdx_active_list_candidates",
        "r8_active_list_candidates",
        "r9_active_list_candidates",
    ]:
        values = []
        for edge in active_edge.get(key) or []:
            if not isinstance(edge, dict):
                continue
            if len(values) >= 12:
                break
            values.append(compact_edge(edge) or edge)
        out[key] = values
    return out


def hex_slice(hex_value: str | None, start: int, end: int) -> str | None:
    if not isinstance(hex_value, str) or end <= start:
        return None
    needed = end * 2
    if len(hex_value) < needed:
        return None
    return hex_value[start * 2 : end * 2]


def row_getter_to_rows(row_getter: dict[str, Any]) -> list[dict[str, Any]]:
    y = row_getter.get("y_arg")
    if y is None:
        return []
    clip_x0 = row_getter.get("clip_x0_0xd8")
    try:
        current_x = int(clip_x0 or 0)
    except (TypeError, ValueError):
        current_x = 0
    scratch_hex = row_getter.get("scratch_prefix_192") or row_getter.get("scratch_prefix_64")
    rows: list[dict[str, Any]] = []
    for cell in row_getter.get("span_buffer_cells") or []:
        if not isinstance(cell, dict):
            continue
        span_type = cell.get("span_type_s32")
        end_x = cell.get("end_x_s32")
        if span_type not in (0, 1, 2) or end_x is None:
            break
        try:
            end_x_int = int(end_x)
        except (TypeError, ValueError):
            break
        if end_x_int >= 0x00FF_FFFF:
            break
        if end_x_int < current_x:
            break
        row = {
            "schema": "ae-native-renderer.are-row-getter-row.v1",
            "source": "ARE_row_getter_8230",
            "span_type": span_type,
            "y": int(y),
            "start_x": current_x,
            "end_x": end_x_int,
            "coverage_len": max(0, end_x_int - current_x),
        }
        if span_type == 2:
            row["coverage_hex"] = hex_slice(scratch_hex, current_x, end_x_int)
            row["coverage_sample_hex"] = row["coverage_hex"]
        rows.append(row)
        current_x = end_x_int
    return rows


def merge_ink_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    merged: list[dict[str, Any]] = []
    current: dict[str, Any] | None = None
    for row in sorted(rows, key=lambda item: (int(item.get("y") or 0), int(item.get("start_x") or 0))):
        if row.get("span_type") not in (1, 2):
            current = None
            continue
        y = int(row["y"])
        start = int(row["start_x"])
        end = int(row["end_x"])
        if current is None or current["y"] != y or current["end_x"] != start:
            current = {
                "schema": "ae-native-renderer.are-row-getter-ink-row.v1",
                "source": "ARE_row_getter_8230",
                "y": y,
                "start_x": start,
                "end_x": end,
                "parts": [],
            }
            merged.append(current)
        else:
            current["end_x"] = end
        current["parts"].append(
            {
                "span_type": row.get("span_type"),
                "start_x": start,
                "end_x": end,
                "coverage_hex": row.get("coverage_hex"),
                "coverage_sample_hex": row.get("coverage_sample_hex"),
            }
        )
    return merged


def analyze(path: Path, event_limit: int) -> dict[str, Any]:
    counts: Counter[tuple[str | None, str | None]] = Counter()
    hook_counts: Counter[str | None] = Counter()
    edge_project_pairs: list[dict[str, Any]] = []
    eval_rows: list[dict[str, Any]] = []
    lazy_rows: list[dict[str, Any]] = []
    prepares: list[dict[str, Any]] = []
    row_getters: list[dict[str, Any]] = []
    row_getter_rows: list[dict[str, Any]] = []
    edge_inserts: list[dict[str, Any]] = []
    active_edge_events: list[dict[str, Any]] = []
    event_builders: list[dict[str, Any]] = []
    event_writes: list[dict[str, Any]] = []
    command_queue_events: list[dict[str, Any]] = []

    edge_projection_ranges: Counter[tuple[Any, Any, Any, Any, Any, Any]] = Counter()
    coverage_values: Counter[tuple[Any, Any, Any, Any, Any]] = Counter()
    row_getter_runs: Counter[tuple[Any, Any, Any, Any]] = Counter()

    enter_edges_by_ptr: dict[str, dict[str, Any]] = {}

    for event in payloads(path):
        kind = event.get("kind")
        hook = event.get("hook")
        if not hook:
            continue
        counts[(hook, kind)] += 1
        hook_counts[hook] += 1
        if kind not in {"cooltype_hook_enter", "cooltype_hook_leave"}:
            continue

        phase = "enter" if kind == "cooltype_hook_enter" else "leave"
        sampler_key = "are_sampler" if phase == "enter" else "are_sampler_after"
        sampler_payload = event.get(sampler_key)
        if not isinstance(sampler_payload, dict):
            continue

        if hook == "ARE_edge_project_78e4":
            edge = dig(sampler_payload, "edge_project", "edge_arg1_before")
            compact = compact_edge(edge)
            ptr_value = compact.get("ptr") if isinstance(compact, dict) else None
            if phase == "enter" and isinstance(ptr_value, str):
                enter_edges_by_ptr[ptr_value] = compact or {}
            if phase == "leave" and isinstance(ptr_value, str):
                before = enter_edges_by_ptr.pop(ptr_value, None)
                after = compact or {}
                pair = {
                    "seq": event.get("seq"),
                    "edge_ptr": ptr_value,
                    "before": before,
                    "after": after,
                }
                edge_project_pairs.append(pair)
                if before and after:
                    edge_projection_ranges[
                        (
                            before.get("x0"),
                            before.get("y0"),
                            before.get("x1"),
                            before.get("y1"),
                            after.get("projected_min"),
                            after.get("projected_max"),
                        )
                    ] += 1

        elif hook == "ARE_sampler_eval_row_75d0":
            row_arg = dig(sampler_payload, "eval_row", "row_arg2_s32")
            sampler = sampler_summary(dig(sampler_payload, "eval_row", "sampler"))
            if sampler is not None:
                event_out = compact_event(event, phase, {"row_arg2_s32": row_arg, "sampler": sampler})
                if len(eval_rows) < event_limit:
                    eval_rows.append(event_out)
                coverage_values[
                    (
                        phase,
                        row_arg,
                        sampler.get("coverage_accum_0x264_s32"),
                        sampler.get("next_event_fixed_0x268_s32"),
                        sampler.get("row_state_0x190_s64"),
                    )
                ] += 1

        elif hook == "ARE_raster_lazy_row_b7e0":
            row_arg = dig(sampler_payload, "lazy_row", "row_arg2_s32")
            sampler = sampler_summary(dig(sampler_payload, "lazy_row", "sampler"))
            if sampler is not None:
                event_out = compact_event(event, phase, {"row_arg2_s32": row_arg, "sampler": sampler})
                if len(lazy_rows) < event_limit:
                    lazy_rows.append(event_out)

        elif hook == "ARE_sampler_prepare_76dc":
            sampler = sampler_summary(dig(sampler_payload, "prepare", "sampler"))
            if sampler is not None and len(prepares) < event_limit:
                prepares.append(compact_event(event, phase, {"sampler": sampler}))

        elif hook == "ARE_row_getter_8230":
            row_getter = row_getter_summary(sampler_payload.get("row_getter"))
            if row_getter is not None:
                if len(row_getters) < event_limit:
                    row_getters.append(compact_event(event, phase, {"row_getter": row_getter}))
                if phase == "leave":
                    row_getter_rows.extend(row_getter_to_rows(row_getter))
                node = row_getter.get("selected_row_head_node") or {}
                row_getter_runs[
                    (
                        phase,
                        row_getter.get("y_arg"),
                        node.get("x_start_0x00_s32"),
                        node.get("len_0x04_s32"),
                    )
                ] += 1

        elif hook == "ARE_edge_insert_sorted_a850":
            edge = dig(sampler_payload, "edge_insert", "node_arg3_before")
            if len(edge_inserts) < event_limit:
                edge_inserts.append(compact_event(event, phase, {"node_arg3": compact_edge(edge)}))

        elif hook in {"ARE_active_edge_bucket_insert_726c", "ARE_active_edge_drain_b944"}:
            active_edge = compact_active_edge_hook(sampler_payload.get("active_edge"))
            if active_edge is not None and len(active_edge_events) < event_limit:
                active_edge_events.append(compact_event(event, phase, {"active_edge": active_edge}))

        elif hook == "ARE_bucket_insert_or_update_430c":
            builder = sampler_payload.get("event_builder")
            if isinstance(builder, dict) and len(event_builders) < event_limit:
                event_builders.append(compact_event(event, phase, {"event_builder": builder}))

        elif isinstance(hook, str) and hook.startswith("ARE_event_"):
            event_writer = sampler_payload.get("event_writer")
            if isinstance(event_writer, dict) and len(event_writes) < event_limit:
                event_writes.append(compact_event(event, phase, {"event_writer": event_writer}))

        elif hook in {
            "ARE_contour_sibling_merge_7348",
            "ARE_close_finalize_7d20",
            "ARE_edge_vector_append_4afc",
            "ARE_row_bucket_add_71f4",
        }:
            command_queue = compact_command_queue(sampler_payload.get("command_queue"))
            if command_queue is not None and len(command_queue_events) < event_limit:
                command_queue_events.append(
                    compact_event(event, phase, {"command_queue": command_queue})
                )

    row_getter_ink_rows = merge_ink_rows(row_getter_rows)
    return {
        "schema": "ae-native-renderer.are-sampler-trace-analysis.v1",
        "source": "scripts/analyze_are_sampler_trace.py",
        "trace_path": str(path),
        "hook_counts": counter_json(hook_counts),
        "kind_counts": counter_json(counts),
        "edge_projection_range_counts": counter_json(edge_projection_ranges, 64),
        "coverage_state_counts": counter_json(coverage_values, 64),
        "row_getter_run_counts": counter_json(row_getter_runs, 64),
        "row_getter_rows": row_getter_rows,
        "row_getter_ink_rows": row_getter_ink_rows,
        "edge_project_pairs": edge_project_pairs[:event_limit],
        "eval_rows": eval_rows,
        "lazy_rows": lazy_rows,
        "prepare_events": prepares,
        "row_getters": row_getters,
        "edge_inserts": edge_inserts,
        "active_edge_events": active_edge_events,
        "event_builders": event_builders,
        "event_writes": event_writes,
        "command_queue_events": command_queue_events,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace", type=Path)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--events", type=int, default=32)
    args = parser.parse_args()

    result = analyze(args.trace, args.events)
    text = json.dumps(result, indent=2, ensure_ascii=False) + "\n"
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text, encoding="utf-8")
        print(
            json.dumps(
                {
                    "out": str(args.out),
                    "edge_project_pairs": len(result["edge_project_pairs"]),
                    "eval_rows": len(result["eval_rows"]),
                    "row_getters": len(result["row_getters"]),
                    "active_edge_events": len(result["active_edge_events"]),
                    "event_builders": len(result["event_builders"]),
                    "event_writes": len(result["event_writes"]),
                    "command_queue_events": len(result["command_queue_events"]),
                }
            )
        )
    else:
        print(text, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
