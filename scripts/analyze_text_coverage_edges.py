#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any


def row_bytes_ae(row: dict[str, Any]) -> list[int | None]:
    out: list[int | None] = []
    for part in row.get("parts") or []:
        start_x = int(part.get("start_x") or 0)
        end_x = int(part.get("end_x") or start_x)
        length = max(0, end_x - start_x)
        span_type = part.get("span_type")
        if span_type == 1:
            out.extend([255] * length)
            continue
        hex_value = part.get("coverage_hex") or ""
        try:
            values = list(bytes.fromhex(hex_value))
        except ValueError:
            values = []
        out.extend(values[:length])
        if len(values) < length:
            out.extend([None] * (length - len(values)))
    return out


def row_bytes_native(row: dict[str, Any]) -> list[int | None]:
    hex_value = row.get("coverage_hex") or row.get("coverage_sample_hex") or ""
    try:
        return list(bytes.fromhex(hex_value))
    except ValueError:
        return []


def normalize_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    usable = [
        row
        for row in rows
        if row.get("y") is not None
        and row.get("start_x") is not None
        and row.get("end_x") is not None
    ]
    if not usable:
        return []
    min_y = min(int(row["y"]) for row in usable)
    min_x = min(int(row["start_x"]) for row in usable)
    out = []
    for row in usable:
        normalized = dict(row)
        normalized["norm_y"] = int(row["y"]) - min_y
        normalized["norm_start_x"] = int(row["start_x"]) - min_x
        normalized["norm_end_x"] = int(row["end_x"]) - min_x
        out.append(normalized)
    return out


def counter_json(counter: Counter[Any], limit: int | None = None) -> list[dict[str, Any]]:
    items = counter.most_common(limit)
    return [{"key": list(key) if isinstance(key, tuple) else key, "count": count} for key, count in items]


def analyze(row_compare: dict[str, Any], example_limit: int) -> dict[str, Any]:
    ae_rows = normalize_rows(row_compare.get("ae", {}).get("ink_rows") or [])
    native_rows = normalize_rows(row_compare.get("native", {}).get("rows") or [])
    by_y_ae: defaultdict[int, list[dict[str, Any]]] = defaultdict(list)
    by_y_native: defaultdict[int, list[dict[str, Any]]] = defaultdict(list)
    for row in ae_rows:
        by_y_ae[int(row["norm_y"])].append(row)
    for row in native_rows:
        by_y_native[int(row["norm_y"])].append(row)

    row_run_count_pairs: Counter[tuple[int, int]] = Counter()
    edge_delta_counts: Counter[tuple[int, int]] = Counter()
    overlap_equal_counts: Counter[tuple[int, int, bool]] = Counter()
    byte_delta_counts: Counter[int] = Counter()
    mismatch_examples: list[dict[str, Any]] = []

    for y in sorted(set(by_y_ae) | set(by_y_native)):
        ae_line = sorted(by_y_ae.get(y, []), key=lambda row: int(row["norm_start_x"]))
        native_line = sorted(by_y_native.get(y, []), key=lambda row: int(row["norm_start_x"]))
        row_run_count_pairs[(len(ae_line), len(native_line))] += 1
        if len(ae_line) != len(native_line):
            if len(mismatch_examples) < example_limit:
                mismatch_examples.append(
                    {
                        "norm_y": y,
                        "kind": "run_count_mismatch",
                        "ae_runs": [
                            [row["norm_start_x"], row["norm_end_x"]] for row in ae_line
                        ],
                        "native_runs": [
                            [row["norm_start_x"], row["norm_end_x"]] for row in native_line
                        ],
                    }
                )
            continue

        for ae_row, native_row in zip(ae_line, native_line):
            ae_start = int(ae_row["norm_start_x"])
            ae_end = int(ae_row["norm_end_x"])
            native_start = int(native_row["norm_start_x"])
            native_end = int(native_row["norm_end_x"])
            start_delta = native_start - ae_start
            end_delta = native_end - ae_end
            edge_delta_counts[(start_delta, end_delta)] += 1

            ae_bytes = row_bytes_ae(ae_row)
            native_bytes = row_bytes_native(native_row)
            overlap_diffs = []
            for x in range(max(ae_start, native_start), min(ae_end, native_end)):
                ae_index = x - ae_start
                native_index = x - native_start
                ae_value = ae_bytes[ae_index] if ae_index < len(ae_bytes) else None
                native_value = (
                    native_bytes[native_index] if native_index < len(native_bytes) else None
                )
                if ae_value != native_value:
                    if ae_value is not None and native_value is not None:
                        byte_delta_counts[native_value - ae_value] += 1
                    overlap_diffs.append(
                        {
                            "x": x,
                            "ae": ae_value,
                            "native": native_value,
                        }
                    )

            equal = not overlap_diffs
            overlap_equal_counts[(start_delta, end_delta, equal)] += 1
            if (not equal or start_delta != 0 or end_delta != 0) and len(mismatch_examples) < example_limit:
                mismatch_examples.append(
                    {
                        "norm_y": y,
                        "kind": "edge_or_byte_mismatch",
                        "ae_run": [ae_start, ae_end],
                        "native_run": [native_start, native_end],
                        "edge_delta": [start_delta, end_delta],
                        "ae_hex_prefix": row_hex_prefix(ae_bytes),
                        "native_hex_prefix": row_hex_prefix(native_bytes),
                        "overlap_diffs": overlap_diffs[:12],
                    }
                )

    return {
        "schema": "ae-native-renderer.text-coverage-edge-analysis.v1",
        "source": "scripts/analyze_text_coverage_edges.py",
        "row_compare_schema": row_compare.get("schema"),
        "case": row_compare.get("case"),
        "ae_ink_rows": len(ae_rows),
        "native_rows": len(native_rows),
        "row_run_count_pairs": counter_json(row_run_count_pairs),
        "edge_delta_counts": counter_json(edge_delta_counts),
        "overlap_equal_counts": counter_json(overlap_equal_counts),
        "byte_delta_counts": counter_json(byte_delta_counts, 24),
        "mismatch_examples": mismatch_examples,
    }


def row_hex_prefix(values: list[int | None], max_values: int = 40) -> str:
    out = []
    for value in values[:max_values]:
        out.append("--" if value is None else f"{value:02x}")
    return "".join(out)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--row-compare", required=True)
    ap.add_argument("--out", default="")
    ap.add_argument("--examples", type=int, default=24)
    args = ap.parse_args()

    row_compare = json.loads(Path(args.row_compare).read_text(encoding="utf-8"))
    result = analyze(row_compare, args.examples)
    text = json.dumps(result, indent=2, ensure_ascii=False) + "\n"
    if args.out:
        out = Path(args.out)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")
        print(json.dumps({"out": str(out), "ae_ink_rows": result["ae_ink_rows"], "native_rows": result["native_rows"]}))
    else:
        print(text, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
