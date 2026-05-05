#!/usr/bin/env python3
"""Summarize AE Frida trace folders into a compact module/path report."""

from __future__ import annotations

import argparse
import collections
import json
import string
from pathlib import Path
from typing import Any


def load_events(path: Path) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if event.get("type") == "send":
                payload = event.get("payload")
                if isinstance(payload, dict):
                    event = payload
            if isinstance(event, dict):
                events.append(event)
    return events


def summarize_events(case_id: str, events: list[dict[str, Any]]) -> dict[str, Any]:
    kinds = collections.Counter(event.get("kind") for event in events)
    render_kinds = collections.Counter(
        event.get("render_kind")
        for event in events
        if event.get("kind") == "render_export_enter" and event.get("render_kind")
    )
    generic_exports = collections.Counter(
        (event.get("module"), event.get("export_name"))
        for event in events
        if event.get("kind") == "generic_export_enter"
    )
    offset_hooks = collections.Counter(
        (event.get("module"), event.get("offset"), event.get("label"))
        for event in events
        if event.get("kind") == "offset_hook_enter"
    )
    effect_proc_offsets = collections.Counter(
        (event.get("module"), event.get("offset"), event.get("origin"))
        for event in events
        if event.get("kind") == "effect_proc_enter"
    )
    cpu_exports = collections.Counter(
        (event.get("module"), event.get("export_name"))
        for event in events
        if event.get("kind") == "cpu_effect_enter"
    )
    stalker_calls = collections.Counter()
    stalker_summary_count = 0
    for event in events:
        if event.get("kind") != "stalker_call_summary":
            continue
        stalker_summary_count += 1
        items = event.get("items")
        if not isinstance(items, list):
            continue
        for item in items:
            if not isinstance(item, dict):
                continue
            try:
                count = int(item.get("count") or 0)
            except (TypeError, ValueError):
                count = 0
            stalker_calls[(item.get("module"), item.get("offset"))] += count
    modules = collections.Counter(
        event.get("module") for event in events if event.get("module")
    )
    effect_proc_pf_cmds = collections.Counter()
    effect_proc_param_sets: list[list[str]] = []
    effect_proc_nonnull_params = 0
    for event in events:
        if event.get("kind") != "effect_proc_enter":
            continue
        call = event.get("ae_effect_call")
        if not isinstance(call, dict):
            continue
        effect_proc_pf_cmds[call.get("pf_cmd_s32")] += 1
        param_defs = call.get("param_defs")
        if not isinstance(param_defs, list) or not param_defs:
            continue
        effect_proc_nonnull_params += 1
        names = []
        for param in param_defs:
            if not isinstance(param, dict):
                continue
            name = decode_param_name(param)
            if name:
                names.append(f"{param.get('index')}:{name}")
        if names and names not in effect_proc_param_sets:
            effect_proc_param_sets.append(names)

    return {
        "case_id": case_id,
        "event_count": len(events),
        "kinds": kinds,
        "render_kinds": render_kinds,
        "generic_exports": generic_exports,
        "offset_hooks": offset_hooks,
        "effect_proc_offsets": effect_proc_offsets,
        "effect_proc_pf_cmds": effect_proc_pf_cmds,
        "effect_proc_nonnull_params": effect_proc_nonnull_params,
        "effect_proc_param_sets": effect_proc_param_sets[:4],
        "cpu_exports": cpu_exports,
        "stalker_summary_count": stalker_summary_count,
        "stalker_calls": stalker_calls,
        "modules": modules,
    }


def decode_ptr_ascii(value: Any) -> str:
    if not isinstance(value, str) or not value.startswith("0x"):
        return ""
    try:
        raw = int(value, 16).to_bytes(8, "little", signed=False)
    except ValueError:
        return ""
    chars = []
    for byte in raw:
        if byte == 0:
            break
        char = chr(byte)
        if char not in string.printable:
            break
        chars.append(char)
    return "".join(chars)


def decode_param_name(param: dict[str, Any]) -> str:
    definition = param.get("def")
    if not isinstance(definition, dict):
        return ""
    words = definition.get("words")
    if not isinstance(words, list):
        return ""
    chunks = []
    for word in words:
        if not isinstance(word, dict):
            continue
        text = decode_ptr_ascii(word.get("ptr"))
        if text:
            chunks.append(text)
    joined = "".join(chunks)
    cleaned = "".join(ch for ch in joined if ch in string.printable).strip()
    return cleaned[:80]


def summarize_summary_case(case: dict[str, Any]) -> dict[str, Any]:
    events: list[dict[str, Any]] = []
    for key in (
        "render_export_enter",
        "generic_export_enter",
        "offset_hook_enter",
        "effect_proc_enter",
        "cpu_effect_enter",
        "stalker_call_summary",
    ):
        value = case.get(key)
        if isinstance(value, list):
            events.extend(event for event in value if isinstance(event, dict))
    summary = summarize_events(str(case.get("case_id", "unknown")), events)
    summary["event_count"] = case.get("event_count", summary["event_count"])
    return summary


def top_items(counter: collections.Counter, limit: int) -> str:
    parts = []
    for item, count in counter.most_common(limit):
        if isinstance(item, tuple):
            label = " ".join(str(part) for part in item if part)
        else:
            label = str(item)
        parts.append(f"{label} x{count}")
    return "; ".join(parts) if parts else "-"


def has_gpu_render_path(render_kinds: collections.Counter) -> bool:
    names = set(render_kinds)
    return any(
        token in names
        for token in (
            "gf_transform_operation_ctor",
            "gf_transform_operation_quality",
            "gf_transform_with_motion_blur",
        )
    )


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Summarize AE Frida JSONL/summary traces for renderer reverse work."
    )
    parser.add_argument("trace_dir", type=Path)
    parser.add_argument("--top", type=int, default=8)
    args = parser.parse_args()

    summaries: list[dict[str, Any]] = []
    jsonl_paths = sorted(args.trace_dir.glob("*.jsonl"))
    summary_path = args.trace_dir / "summary.json"
    if jsonl_paths:
        for path in jsonl_paths:
            summaries.append(summarize_events(path.stem, load_events(path)))
    elif summary_path.exists():
        data = json.loads(summary_path.read_text(encoding="utf-8"))
        for case in data.get("cases", []):
            if isinstance(case, dict):
                summaries.append(summarize_summary_case(case))

    print(f"# Frida Trace Summary: {args.trace_dir}")
    for summary in summaries:
        render_kinds = summary["render_kinds"]
        print()
        print(f"## {summary['case_id']}")
        print(f"- events: {summary['event_count']}")
        print(f"- gpu_render_path_seen: {str(has_gpu_render_path(render_kinds)).lower()}")
        print(f"- render exports: {top_items(render_kinds, args.top)}")
        print(f"- effect procs: {top_items(summary['effect_proc_offsets'], args.top)}")
        print(f"- effect PF cmds: {top_items(summary['effect_proc_pf_cmds'], args.top)}")
        print(f"- effect render-param calls: {summary['effect_proc_nonnull_params']}")
        if summary["effect_proc_param_sets"]:
            print(f"- effect param sets: {summary['effect_proc_param_sets']}")
        print(f"- offset hooks: {top_items(summary['offset_hooks'], args.top)}")
        print(f"- cpu exports: {top_items(summary['cpu_exports'], args.top)}")
        print(f"- stalker summaries: {summary['stalker_summary_count']}")
        print(f"- stalker calls: {top_items(summary['stalker_calls'], args.top)}")
        print(f"- generic exports: {top_items(summary['generic_exports'], args.top)}")
        print(f"- modules: {top_items(summary['modules'], args.top)}")


if __name__ == "__main__":
    main()
