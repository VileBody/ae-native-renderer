#!/usr/bin/env python3
"""Prepare focused Ghidra reverse bundles for AE parity agents."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = REPO_ROOT / "docs/reverse_engineering/GHIDRA_PREDECODE_TASKS.json"
DEFAULT_SCRIPT_PATH = REPO_ROOT / "scripts/ghidra"
DEFAULT_GHIDRA_HOME = Path(os.environ.get("GHIDRA_HOME", "/Applications/ghidra_12.0_PUBLIC"))
DEFAULT_LOCK_DIR = Path(os.environ.get("GHIDRA_LOCK_DIR", "/tmp/ae-native-renderer-ghidra.lockdir"))


def slugify(value: str) -> str:
    value = re.sub(r"[^A-Za-z0-9._-]+", "_", value.strip())
    value = re.sub(r"_+", "_", value).strip("_")
    return value[:96] or "run"


class DirectoryLock:
    def __init__(self, path: Path, enabled: bool = True):
        self.path = path
        self.enabled = enabled
        self.acquired = False

    def __enter__(self) -> "DirectoryLock":
        if not self.enabled:
            return self
        while True:
            try:
                self.path.mkdir()
                self.acquired = True
                return self
            except FileExistsError:
                time.sleep(1)

    def __exit__(self, exc_type: Any, exc: Any, tb: Any) -> None:
        if self.acquired:
            try:
                self.path.rmdir()
            except OSError:
                pass


def load_manifest(path: Path) -> list[dict[str, Any]]:
    data = json.loads(path.read_text())
    return data["tasks"]


def selected_tasks(tasks: list[dict[str, Any]], args: argparse.Namespace) -> list[dict[str, Any]]:
    task_ids = set(args.task or [])
    areas = set(args.area or [])
    if args.all:
        selected = tasks
    elif task_ids or areas:
        selected = []
        for task in tasks:
            area = task.get("area", "")
            if task["id"] in task_ids or area in areas or any(area.startswith(prefix) for prefix in areas):
                selected.append(task)
    else:
        selected = []

    if not args.include_disabled:
        selected = [task for task in selected if task.get("enabled", True)]
    return selected


def print_task_list(tasks: list[dict[str, Any]]) -> None:
    print("Ghidra predecode tasks:")
    for task in tasks:
        status = "enabled" if task.get("enabled", True) else "disabled"
        print(f"  {task['id']:<34} {task.get('area', ''):<18} {status:<8} {task.get('process', '')}")


def target_specs(task: dict[str, Any]) -> list[str]:
    specs = []
    for target in task.get("targets", []):
        specs.append(f"{target['label']}@{target['address']}")
    return specs


def task_project_exists(task: dict[str, Any]) -> bool:
    project_dir = REPO_ROOT / task["project_dir"]
    return (project_dir / f"{task['project']}.gpr").exists()


def run_task(task: dict[str, Any], args: argparse.Namespace, run_root: Path) -> dict[str, Any]:
    project_dir = REPO_ROOT / task["project_dir"]
    task_out = run_root / task["id"]
    task_out.mkdir(parents=True, exist_ok=True)

    command = [
        str(args.ghidra_home / "support/analyzeHeadless"),
        str(project_dir),
        task["project"],
        "-process",
        task["process"],
        "-noanalysis",
        "-postScript",
        "PreparedReverseBundle.java",
        str(task_out),
        task["id"],
        *target_specs(task),
        "-scriptPath",
        str(args.script_path),
        "-log",
        str(task_out / "ghidra.log"),
    ]

    record: dict[str, Any] = {
        "id": task["id"],
        "area": task.get("area"),
        "process": task.get("process"),
        "output": str(task_out.relative_to(REPO_ROOT)),
        "command": command,
        "started_at": dt.datetime.now().isoformat(timespec="seconds"),
    }

    print(f"[ghidra] {task['id']} -> {task_out.relative_to(REPO_ROOT)}")
    if args.dry_run:
        record["status"] = "dry_run"
        return record

    result = subprocess.run(command, cwd=REPO_ROOT, text=True, capture_output=True)
    (task_out / "stdout.txt").write_text(result.stdout)
    (task_out / "stderr.txt").write_text(result.stderr)

    record["returncode"] = result.returncode
    record["finished_at"] = dt.datetime.now().isoformat(timespec="seconds")
    record["status"] = "ok" if result.returncode == 0 else "failed"
    return record


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--ghidra-home", type=Path, default=DEFAULT_GHIDRA_HOME)
    parser.add_argument("--script-path", type=Path, default=DEFAULT_SCRIPT_PATH)
    parser.add_argument("--task", action="append", help="Task id to run. May be repeated.")
    parser.add_argument("--area", action="append", help="Area name/prefix to run. May be repeated.")
    parser.add_argument("--all", action="store_true", help="Run all enabled tasks.")
    parser.add_argument("--include-disabled", action="store_true", help="Allow disabled manifest tasks.")
    parser.add_argument("--skip-missing", action="store_true", help="Skip tasks whose Ghidra project is not present.")
    parser.add_argument("--list", action="store_true", help="List known tasks and exit.")
    parser.add_argument("--dry-run", action="store_true", help="Print planned commands without running Ghidra.")
    parser.add_argument("--no-lock", action="store_true", help="Do not take the shared Ghidra directory lock.")
    parser.add_argument("--out-root", type=Path, default=REPO_ROOT / "target/reverse/predecoded")
    parser.add_argument("--run-name", default=None)
    args = parser.parse_args()

    tasks = load_manifest(args.manifest)
    if args.list:
        print_task_list(tasks)
        return 0

    selected = selected_tasks(tasks, args)
    if not selected:
        print_task_list(tasks)
        print("\nNo task selected. Use --task, --area, or --all.")
        return 0

    analyze_headless = args.ghidra_home / "support/analyzeHeadless"
    if not analyze_headless.exists():
        print(f"Missing analyzeHeadless: {analyze_headless}", file=sys.stderr)
        return 2
    if not (args.script_path / "PreparedReverseBundle.java").exists():
        print(f"Missing PreparedReverseBundle.java under {args.script_path}", file=sys.stderr)
        return 2

    runnable = []
    missing = []
    for task in selected:
        if task_project_exists(task):
            runnable.append(task)
        else:
            missing.append(task)
    if missing and not args.skip_missing:
        for task in missing:
            print(f"Missing project for task {task['id']}: {task['project_dir']}/{task['project']}.gpr", file=sys.stderr)
        return 2
    if missing:
        for task in missing:
            print(f"[skip] missing project for {task['id']}")

    run_name = args.run_name or "_".join(task["id"] for task in runnable[:3])
    timestamp = dt.datetime.now().strftime("%Y%m%d_%H%M%S")
    run_root = args.out_root / f"{timestamp}_{slugify(run_name)}"
    run_root.mkdir(parents=True, exist_ok=True)

    records = []
    with DirectoryLock(DEFAULT_LOCK_DIR, enabled=not args.no_lock):
        for task in runnable:
            records.append(run_task(task, args, run_root))

    manifest_out = {
        "created_at": dt.datetime.now().isoformat(timespec="seconds"),
        "run_root": str(run_root.relative_to(REPO_ROOT)),
        "tasks": records,
    }
    (run_root / "run_manifest.json").write_text(json.dumps(manifest_out, indent=2))

    failed = [record for record in records if record.get("status") == "failed"]
    print(f"\nWrote {run_root.relative_to(REPO_ROOT)}")
    if failed:
        print("Failed tasks: " + ", ".join(record["id"] for record in failed), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
