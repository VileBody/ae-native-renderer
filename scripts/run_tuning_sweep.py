#!/usr/bin/env python3
"""Render one request across runtime tuning values without recompiling Rust."""

from __future__ import annotations

import argparse
import copy
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_RENDER_CLI = ROOT / "target" / "release" / "render-cli"


def parse_value(raw: str) -> Any:
    try:
        return json.loads(raw)
    except json.JSONDecodeError as error:
        raise argparse.ArgumentTypeError(f"value must be valid JSON: {raw!r}") from error


def set_nested(target: dict[str, Any], dotted_path: str, value: Any) -> None:
    parts = [part for part in dotted_path.split(".") if part]
    if not parts:
        raise ValueError("parameter path cannot be empty")
    current = target
    for part in parts[:-1]:
        child = current.setdefault(part, {})
        if not isinstance(child, dict):
            raise ValueError(f"parameter path crosses non-object field: {part}")
        current = child
    current[parts[-1]] = value


def case_slug(index: int, value: Any) -> str:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":"))
    safe = "".join(character if character.isalnum() else "_" for character in encoded)
    return f"{index:02d}_{safe.strip('_') or 'value'}"[:80]


def build_case_request(
    base: dict[str, Any],
    profile: str,
    parameter: str,
    value: Any,
    output_directory: Path,
    index: int,
) -> dict[str, Any]:
    request = copy.deepcopy(base)
    tuning = request.setdefault("tuningSpec", {})
    if not isinstance(tuning, dict):
        raise ValueError("request tuningSpec must be an object")
    tuning["profile"] = profile
    overrides = tuning.setdefault("overrides", {})
    if not isinstance(overrides, dict):
        raise ValueError("request tuningSpec.overrides must be an object")
    set_nested(overrides, parameter, value)
    output = request.setdefault("outputSpec", {})
    if not isinstance(output, dict):
        raise ValueError("request outputSpec must be an object")
    output["directory"] = str(output_directory.resolve())
    request_id = request.get("requestId") or "tuning-sweep"
    request["requestId"] = f"{request_id}-tune-{index:02d}"
    return request


def run_sweep(args: argparse.Namespace) -> Path:
    request_path = args.request.resolve()
    request_dir = request_path.parent
    base = json.loads(request_path.read_text(encoding="utf-8"))
    if not isinstance(base, dict):
        raise SystemExit("request root must be an object")
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    render_cli = args.render_cli.resolve()
    if not args.dry_run and not render_cli.is_file():
        raise SystemExit(
            f"render-cli binary is missing: {render_cli}; build it once with "
            "cargo build --release -p render-cli"
        )

    cases: list[dict[str, Any]] = []
    for index, value in enumerate(args.value, start=1):
        case_dir = out / case_slug(index, value)
        case_dir.mkdir(parents=True, exist_ok=True)
        request = build_case_request(
            base,
            args.profile,
            args.parameter,
            value,
            case_dir / "render-output",
            index,
        )
        generated_request = case_dir / "request.json"
        generated_request.write_text(
            json.dumps(request, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        row: dict[str, Any] = {
            "index": index,
            "value": value,
            "request": str(generated_request.relative_to(out)),
            "output": str((case_dir / "render-output").relative_to(out)),
            "status": "planned" if args.dry_run else "running",
        }
        if not args.dry_run:
            completed = subprocess.run(
                [str(render_cli), "json", "--request", "-"],
                input=json.dumps(request),
                text=True,
                cwd=request_dir,
                capture_output=True,
                check=False,
            )
            response_path = case_dir / "response.json"
            response_path.write_text(completed.stdout, encoding="utf-8")
            (case_dir / "stderr.log").write_text(completed.stderr, encoding="utf-8")
            try:
                response = json.loads(completed.stdout)
            except json.JSONDecodeError:
                response = {}
            row.update(
                {
                    "exit_code": completed.returncode,
                    "status": response.get("status", "protocol_error"),
                    "ok": response.get("ok", False),
                    "complete": response.get("complete", False),
                    "response": str(response_path.relative_to(out)),
                    "artifacts": response.get("artifacts", {}),
                }
            )
        cases.append(row)
        print(f"[{index}/{len(args.value)}] {value!r}: {row['status']}", flush=True)

    report = {
        "schema": "ae-native-renderer.tuning-sweep.v1",
        "source_request": str(request_path),
        "profile": args.profile,
        "parameter": args.parameter,
        "dry_run": args.dry_run,
        "cases": cases,
    }
    report_path = out / "sweep-index.json"
    report_path.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return report_path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--request", type=Path, required=True)
    parser.add_argument("--parameter", required=True, help="Dotted tuning override path")
    parser.add_argument("--value", action="append", type=parse_value, required=True)
    parser.add_argument("--profile", default="builtin:p0p1-readiness")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--render-cli", type=Path, default=DEFAULT_RENDER_CLI)
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args()


if __name__ == "__main__":
    try:
        print(run_sweep(parse_args()))
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"tuning sweep failed: {error}", file=sys.stderr)
        raise SystemExit(2) from error
