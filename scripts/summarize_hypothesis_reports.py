#!/usr/bin/env python3
import argparse
import json
from pathlib import Path


def load_reports(root: Path) -> list[dict]:
    reports = []
    for path in sorted(root.glob("**/hypothesis_report.json")):
        try:
            report = json.loads(path.read_text())
        except Exception as exc:  # noqa: BLE001
            reports.append(
                {
                    "path": str(path),
                    "module": "<parse-error>",
                    "candidate_id": path.parent.name,
                    "status": "parse_error",
                    "accepted": False,
                    "rejected": False,
                    "needs_new_probe": True,
                    "gate_ok": False,
                    "cases": [],
                    "error": str(exc),
                }
            )
            continue

        gates = report.get("gates") or []
        gate_ok = bool(gates) and all(bool(gate.get("ok")) for gate in gates)
        cases = []
        for gate in gates:
            cases.extend(gate.get("cases") or [])
        reports.append(
            {
                "path": str(path),
                "module": report.get("module", ""),
                "candidate_id": report.get("candidate_id", ""),
                "status": report.get("status", ""),
                "accepted": bool(report.get("accepted")),
                "rejected": bool(report.get("rejected")),
                "needs_new_probe": bool(report.get("needs_new_probe")),
                "gate_ok": gate_ok,
                "cases": sorted(set(cases)),
                "question": report.get("question"),
                "hypothesis": report.get("hypothesis"),
            }
        )
    return reports


def print_markdown(reports: list[dict]) -> None:
    print("| Module | Candidate | Status | Gate OK | Decision | Cases | Report |")
    print("| --- | --- | --- | --- | --- | --- | --- |")
    for report in reports:
        decision = "pending"
        if report["accepted"]:
            decision = "accepted"
        elif report["rejected"]:
            decision = "rejected"
        elif report["needs_new_probe"]:
            decision = "needs_new_probe"
        print(
            "| {module} | {candidate_id} | {status} | {gate_ok} | {decision} | {cases} | {path} |".format(
                module=report["module"],
                candidate_id=report["candidate_id"],
                status=report["status"],
                gate_ok=str(report["gate_ok"]).lower(),
                decision=decision,
                cases=", ".join(report["cases"]),
                path=report["path"],
            )
        )


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Summarize render-cli hypothesis-pack reports."
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path("target/ae_agents"),
        help="Root directory to scan for hypothesis_report.json files.",
    )
    parser.add_argument(
        "--json",
        type=Path,
        default=None,
        help="Optional JSON output path.",
    )
    args = parser.parse_args()

    reports = load_reports(args.root)
    print_markdown(reports)
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps({"reports": reports}, indent=2))


if __name__ == "__main__":
    main()
