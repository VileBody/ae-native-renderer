#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


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


def delta(before: Any, after: Any) -> str:
    b = number(before)
    a = number(after)
    if b is None or a is None:
        return ""
    sign = "+" if a - b > 0 else ""
    return f"{sign}{a - b:.6g}"


def fmt(value: Any) -> str:
    if value is None:
        return "-"
    if isinstance(value, float):
        return f"{value:.6g}"
    return str(value)


def case_map(report: dict[str, Any]) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for case in report.get("cases", []):
        case_id = case.get("case") or case.get("id")
        if case_id:
            out[str(case_id)] = case
    return out


def load(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as f:
        return json.load(f)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Compare two render-cli conformance-pack report.json files."
    )
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument(
        "--json",
        action="store_true",
        help="emit machine-readable JSON instead of a markdown table",
    )
    args = parser.parse_args()

    baseline = case_map(load(args.baseline))
    candidate = case_map(load(args.candidate))
    keys = sorted(set(baseline) | set(candidate))
    fields = [
        ("mean", "mean_abs_diff"),
        ("rgb_mean", "metrics.rgb.mean_abs_diff"),
        ("max", "max_abs_diff"),
        ("text_mismatches", "text_passport.total_mismatches"),
        ("text_max_delta", "text_passport.max_abs_delta"),
        ("text_compared", "text_passport.compared_frames"),
    ]

    rows: list[dict[str, Any]] = []
    for key in keys:
        before = baseline.get(key, {}).get("summary", {})
        after = candidate.get(key, {}).get("summary", {})
        row: dict[str, Any] = {"case": key}
        for label, path in fields:
            b = metric(before, path)
            a = metric(after, path)
            row[f"{label}_before"] = b
            row[f"{label}_after"] = a
            row[f"{label}_delta"] = number(a) - number(b) if number(a) is not None and number(b) is not None else None
        rows.append(row)

    if args.json:
        print(json.dumps({"rows": rows}, indent=2, sort_keys=True))
        return 0

    headers = [
        "case",
        "mean",
        "delta",
        "rgb_mean",
        "delta",
        "max",
        "delta",
        "text_mismatches",
        "delta",
        "text_max_delta",
        "delta",
    ]
    print("| " + " | ".join(headers) + " |")
    print("| " + " | ".join(["---"] + ["---:"] * (len(headers) - 1)) + " |")
    for row in rows:
        print(
            "| "
            + " | ".join(
                [
                    row["case"],
                    fmt(row["mean_after"]),
                    delta(row["mean_before"], row["mean_after"]),
                    fmt(row["rgb_mean_after"]),
                    delta(row["rgb_mean_before"], row["rgb_mean_after"]),
                    fmt(row["max_after"]),
                    delta(row["max_before"], row["max_after"]),
                    fmt(row["text_mismatches_after"]),
                    delta(row["text_mismatches_before"], row["text_mismatches_after"]),
                    fmt(row["text_max_delta_after"]),
                    delta(row["text_max_delta_before"], row["text_max_delta_after"]),
                ]
            )
            + " |"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
