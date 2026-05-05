#!/usr/bin/env python3
"""Summarize AE Frida trace folders into a compact module/path report."""

from __future__ import annotations

import argparse
import collections
import json
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
    modules = collections.Counter(
        event.get("module") for event in events if event.get("module")
    )

    return {
        "case_id": case_id,
        "event_count": len(events),
        "kinds": kinds,
        "render_kinds": render_kinds,
        "generic_exports": generic_exports,
        "offset_hooks": offset_hooks,
        "effect_proc_offsets": effect_proc_offsets,
        "cpu_exports": cpu_exports,
        "modules": modules,
    }


def summarize_summary_case(case: dict[str, Any]) -> dict[str, Any]:
    events: list[dict[str, Any]] = []
    for key in (
        "render_export_enter",
        "generic_export_enter",
        "offset_hook_enter",
        "effect_proc_enter",
        "cpu_effect_enter",
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
    summary_path = args.trace_dir / "summary.json"
    if summary_path.exists():
        data = json.loads(summary_path.read_text(encoding="utf-8"))
        for case in data.get("cases", []):
            if isinstance(case, dict):
                summaries.append(summarize_summary_case(case))
    else:
        for path in sorted(args.trace_dir.glob("*.jsonl")):
            summaries.append(summarize_events(path.stem, load_events(path)))

    print(f"# Frida Trace Summary: {args.trace_dir}")
    for summary in summaries:
        render_kinds = summary["render_kinds"]
        print()
        print(f"## {summary['case_id']}")
        print(f"- events: {summary['event_count']}")
        print(f"- gpu_render_path_seen: {str(has_gpu_render_path(render_kinds)).lower()}")
        print(f"- render exports: {top_items(render_kinds, args.top)}")
        print(f"- effect procs: {top_items(summary['effect_proc_offsets'], args.top)}")
        print(f"- offset hooks: {top_items(summary['offset_hooks'], args.top)}")
        print(f"- cpu exports: {top_items(summary['cpu_exports'], args.top)}")
        print(f"- generic exports: {top_items(summary['generic_exports'], args.top)}")
        print(f"- modules: {top_items(summary['modules'], args.top)}")


if __name__ == "__main__":
    main()
