#!/usr/bin/env python3
"""Render the self-contained P0/P1 surface case and compare its committed golden."""

from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
from pathlib import Path

from compare_frame_locked import run_comparison


ROOT = Path(__file__).resolve().parents[1]
REQUEST = ROOT / "fixtures/ci_visual_regression/p0p1_surface.request.json"
GOLDEN = ROOT / "fixtures/ci_visual_regression/golden"


def run(render_cli: Path) -> Path:
    request = json.loads(REQUEST.read_text(encoding="utf-8"))
    with tempfile.TemporaryDirectory(prefix="anr-ci-visual-") as temporary:
        root = Path(temporary)
        render_out = root / "render-output"
        request["outputSpec"]["directory"] = str(render_out)
        completed = subprocess.run(
            [str(render_cli.resolve()), "json", "--request", "-"],
            input=json.dumps(request),
            text=True,
            cwd=ROOT,
            capture_output=True,
            check=False,
        )
        if completed.returncode != 0:
            raise RuntimeError(f"render-cli exited {completed.returncode}: {completed.stderr}")
        response = json.loads(completed.stdout)
        if not response.get("ok") or not response.get("complete"):
            raise RuntimeError(f"visual regression request is incomplete: {completed.stdout}")
        rust_frames = render_out / "render" / "frames"
        metrics = run_comparison(
            rust_dir=rust_frames,
            ae_dir=GOLDEN,
            frame_specs=["0"],
            out_dir=root / "comparison",
        )
        report = json.loads(metrics.read_text(encoding="utf-8"))
        frame = report["frames"][0]["metrics"]
        if not frame["identical"]:
            raise RuntimeError(
                f"visual regression: mae={frame['mae']} max_abs_diff={frame['max_abs_diff']} ssim={frame['ssim']}"
            )
        persisted = ROOT / "target/ci_visual_regression_metrics.json"
        persisted.parent.mkdir(parents=True, exist_ok=True)
        persisted.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        return persisted


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--render-cli", type=Path, required=True)
    args = parser.parse_args()
    print(run(args.render_cli))


if __name__ == "__main__":
    main()
