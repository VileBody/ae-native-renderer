#!/usr/bin/env python3
"""Run or classify the master AE conformance gate.

The native conformance runner owns rendering and frame diffs. This wrapper owns
the product gate: selected cases, per-module roles, threshold bands, and the
template dashboard that answers "accepted / approximate / regression".
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_POLICY = ROOT / "fixtures" / "ae_conformance_pack" / "master_gate_policy.json"
STATUS_ORDER = {
    "missing": 0,
    "recipe_error": 0,
    "regression": 1,
    "tuning": 2,
    "approximate": 3,
    "accepted": 4,
}


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def merged_thresholds(policy: dict[str, Any], case_id: str) -> dict[str, float]:
    values = dict(policy["default_thresholds"])
    values.update(policy.get("case_overrides", {}).get(case_id, {}))
    return values


def nested(report: dict[str, Any], path: list[str], default: Any = None) -> Any:
    current: Any = report
    for key in path:
        if not isinstance(current, dict) or key not in current:
            return default
        current = current[key]
    return current


def case_metric(case_report: dict[str, Any], policy: dict[str, Any]) -> dict[str, Any]:
    summary = case_report.get("summary") or {}
    alpha_diag = summary.get("alpha_policy_diagnostics") or {}
    mean_abs = alpha_diag.get("mean_abs_diff") or {}
    primary_name = policy.get("primary_metric", "rgb_straight_source_over_ae_background")
    primary = mean_abs.get(primary_name)
    raw_rgba = mean_abs.get("raw_rgba", summary.get("mean_abs_diff"))
    alpha = mean_abs.get("alpha")
    background = mean_abs.get("background_alpha_normalized")
    raw_rgb = mean_abs.get("raw_rgb")
    max_abs = summary.get("max_abs_diff")
    temporal_ok = nested(summary, ["temporal_contract", "ok"], True)
    text_ok = nested(summary, ["text_passport", "ok"], True)
    return {
        "primary_metric": primary_name,
        "primary_mean": primary,
        "raw_rgba_mean": raw_rgba,
        "raw_rgb_mean": raw_rgb,
        "alpha_mean": alpha,
        "background_alpha_normalized_mean": background,
        "max_abs_diff": max_abs,
        "changed_pixel_ratio": summary.get("changed_pixel_ratio"),
        "temporal_ok": temporal_ok,
        "text_passport_ok": text_ok,
    }


def classify(metric: dict[str, Any], thresholds: dict[str, float], case_report: dict[str, Any]) -> str:
    if case_report.get("status") == "recipe_error":
        return "recipe_error"
    primary = metric.get("primary_mean")
    max_abs = metric.get("max_abs_diff")
    if primary is None or max_abs is None:
        return "missing"
    if not metric.get("temporal_ok", True):
        return "regression"
    if (
        primary <= thresholds["accepted_mean"]
        and max_abs <= thresholds["accepted_max"]
    ):
        return "accepted"
    if (
        primary <= thresholds["approximate_mean"]
        and max_abs <= thresholds["approximate_max"]
    ):
        return "approximate"
    if primary > thresholds["regression_mean"] or max_abs > thresholds["regression_max"]:
        return "regression"
    return "tuning"


def summarize_status(rows: list[dict[str, Any]]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for row in rows:
        counts[row["status"]] = counts.get(row["status"], 0) + 1
    return counts


def weakest_status(rows: list[dict[str, Any]]) -> str:
    if not rows:
        return "missing"
    return min((row["status"] for row in rows), key=lambda status: STATUS_ORDER.get(status, -1))


def render_report(policy: dict[str, Any], out: Path, extra_cases: list[str], no_render: bool) -> Path:
    pack = Path(policy.get("pack", "fixtures/ae_conformance_pack"))
    if not pack.is_absolute():
        pack = ROOT / pack
    gate_cases = [case["id"] for case in policy["gate_cases"]]
    cases = extra_cases or gate_cases
    if no_render:
        report = out / "report.json"
        if not report.exists():
            raise SystemExit(f"--no-render requested but report is missing: {report}")
        return report

    out.mkdir(parents=True, exist_ok=True)
    cmd = [
        "cargo",
        "run",
        "-p",
        "render-cli",
        "--bin",
        "render-cli",
        "--",
        "conformance-pack",
        "--pack",
        str(pack),
        "--out",
        str(out),
    ]
    for case_id in cases:
        cmd.extend(["--case", case_id])
    print("$ " + " ".join(cmd), flush=True)
    result = subprocess.run(cmd, cwd=ROOT)
    if result.returncode != 0:
        raise SystemExit(result.returncode)
    return out / "report.json"


def build_dashboard(policy: dict[str, Any], report: dict[str, Any]) -> dict[str, Any]:
    by_case_report = {
        case.get("case"): case
        for case in report.get("cases", [])
        if isinstance(case, dict) and case.get("case")
    }
    gate_rows = []
    case_meta_by_id = {case["id"]: case for case in policy["gate_cases"]}
    for case_meta in policy["gate_cases"]:
        case_id = case_meta["id"]
        case_report = by_case_report.get(case_id)
        if case_report is None:
            row = {
                "case": case_id,
                "role": case_meta["role"],
                "modules": case_meta["modules"],
                "templates": case_meta["templates"],
                "status": "missing",
                "metrics": {},
                "thresholds": merged_thresholds(policy, case_id),
            }
        else:
            metrics = case_metric(case_report, policy)
            thresholds = merged_thresholds(policy, case_id)
            row = {
                "case": case_id,
                "title": case_report.get("title"),
                "role": case_meta["role"],
                "modules": case_meta["modules"],
                "templates": case_meta["templates"],
                "status": classify(metrics, thresholds, case_report),
                "native_ok": case_report.get("ok"),
                "metrics": metrics,
                "thresholds": thresholds,
            }
        gate_rows.append(row)

    module_rows = []
    modules = sorted({module for row in gate_rows for module in row["modules"]})
    for module in modules:
        rows = [row for row in gate_rows if module in row["modules"]]
        role_counts = summarize_status(rows)
        module_rows.append(
            {
                "module": module,
                "status": weakest_status(rows),
                "case_count": len(rows),
                "status_counts": role_counts,
                "cases": [row["case"] for row in rows],
            }
        )

    template_rows = []
    for template, case_ids in policy.get("template_dashboards", {}).items():
        rows = [
            row
            for row in gate_rows
            if row["case"] in case_ids or template in row.get("templates", [])
        ]
        rows.sort(key=lambda row: case_ids.index(row["case"]) if row["case"] in case_ids else 999)
        template_rows.append(
            {
                "template": template,
                "status": weakest_status(rows),
                "status_counts": summarize_status(rows),
                "cases": [row["case"] for row in rows],
            }
        )

    return {
        "schema": "ae-native-renderer.master-conformance-dashboard.v1",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "source_report": report.get("out"),
        "source_report_summary": report.get("summary"),
        "primary_metric": policy.get("primary_metric"),
        "summary": {
            "case_count": len(gate_rows),
            "status_counts": summarize_status(gate_rows),
            "regression_count": sum(1 for row in gate_rows if row["status"] == "regression"),
            "missing_count": sum(1 for row in gate_rows if row["status"] in {"missing", "recipe_error"}),
        },
        "cases": gate_rows,
        "modules": module_rows,
        "templates": template_rows,
        "policy": {
            "schema": policy.get("schema"),
            "default_thresholds": policy.get("default_thresholds"),
        },
        "case_policy": case_meta_by_id,
    }


def fmt_num(value: Any) -> str:
    if isinstance(value, (int, float)):
        return f"{value:.6f}"
    if value is None:
        return "-"
    return str(value)


def markdown_table(rows: list[list[str]]) -> str:
    if not rows:
        return ""
    widths = [max(len(row[idx]) for row in rows) for idx in range(len(rows[0]))]
    lines = []
    for index, row in enumerate(rows):
        lines.append("| " + " | ".join(cell.ljust(widths[col]) for col, cell in enumerate(row)) + " |")
        if index == 0:
            lines.append("| " + " | ".join("-" * widths[col] for col in range(len(row))) + " |")
    return "\n".join(lines)


def write_dashboard_markdown(dashboard: dict[str, Any], out: Path) -> None:
    case_rows = [["Case", "Role", "Status", "Primary", "Max", "Modules"]]
    for row in dashboard["cases"]:
        metrics = row.get("metrics") or {}
        case_rows.append(
            [
                row["case"],
                row["role"],
                row["status"],
                fmt_num(metrics.get("primary_mean")),
                fmt_num(metrics.get("max_abs_diff")),
                ",".join(row.get("modules", [])),
            ]
        )

    module_rows = [["Module", "Status", "Cases", "Counts"]]
    for row in dashboard["modules"]:
        counts = ", ".join(f"{key}:{value}" for key, value in sorted(row["status_counts"].items()))
        module_rows.append([row["module"], row["status"], str(row["case_count"]), counts])

    template_rows = [["Template", "Status", "Counts", "Cases"]]
    for row in dashboard["templates"]:
        counts = ", ".join(f"{key}:{value}" for key, value in sorted(row["status_counts"].items()))
        template_rows.append([row["template"], row["status"], counts, ", ".join(row["cases"])])

    content = "\n".join(
        [
            "# Master Conformance Gate",
            "",
            f"Generated: `{dashboard['generated_at']}`",
            "",
            f"Primary metric: `{dashboard['primary_metric']}`",
            "",
            "## Summary",
            "",
            "```json",
            json.dumps(dashboard["summary"], indent=2),
            "```",
            "",
            "## Templates",
            "",
            markdown_table(template_rows),
            "",
            "## Modules",
            "",
            markdown_table(module_rows),
            "",
            "## Cases",
            "",
            markdown_table(case_rows),
            "",
        ]
    )
    out.write_text(content)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--policy", type=Path, default=DEFAULT_POLICY)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--case", action="append", default=[])
    parser.add_argument("--report", type=Path, help="Classify an existing conformance report.")
    parser.add_argument("--no-render", action="store_true", help="Use <out>/report.json instead of rendering.")
    parser.add_argument("--no-fail", action="store_true", help="Always exit 0 after writing dashboards.")
    args = parser.parse_args()

    policy = load_json(args.policy)
    if args.report:
        report_path = args.report
        out = args.out or report_path.parent
    else:
        out = args.out or ROOT / "target" / "ae_agents" / (
            "master_gate_" + datetime.now().strftime("%Y%m%d_%H%M%S")
        )
        report_path = render_report(policy, out, args.case, args.no_render)

    report = load_json(report_path)
    dashboard = build_dashboard(policy, report)
    out.mkdir(parents=True, exist_ok=True)
    dashboard_json = out / "dashboard.json"
    dashboard_md = out / "dashboard.md"
    dashboard_json.write_text(json.dumps(dashboard, indent=2) + "\n")
    write_dashboard_markdown(dashboard, dashboard_md)
    print(f"master-gate.dashboard json={dashboard_json} md={dashboard_md}")
    print(json.dumps(dashboard["summary"], indent=2))
    if args.no_fail:
        return 0
    return 1 if dashboard["summary"]["regression_count"] else 0


if __name__ == "__main__":
    sys.exit(main())
