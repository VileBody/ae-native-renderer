#!/usr/bin/env python3
"""Self-tests for run_tuning_sweep.py."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from run_tuning_sweep import build_case_request, select_best_case, set_nested


class TuningSweepTest(unittest.TestCase):
    def test_set_nested_builds_override_tree(self) -> None:
        target: dict[str, object] = {}
        set_nested(target, "effects.blur.box_radius_multiplier", 1.25)
        self.assertEqual(
            target,
            {"effects": {"blur": {"box_radius_multiplier": 1.25}}},
        )

    def test_case_request_preserves_existing_overrides(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = {
                "requestId": "job",
                "outputSpec": {"directory": "old"},
                "tuningSpec": {"overrides": {"effects": {"glow": {"radius_multiplier": 0.8}}}},
            }
            request = build_case_request(
                base,
                "builtin:p0p1-readiness",
                "effects.blur.box_radius_multiplier",
                1.5,
                Path(temporary) / "out",
                2,
            )
            self.assertEqual(request["requestId"], "job-tune-02")
            self.assertEqual(
                request["tuningSpec"]["overrides"]["effects"]["glow"]["radius_multiplier"],
                0.8,
            )
            self.assertEqual(
                request["tuningSpec"]["overrides"]["effects"]["blur"]["box_radius_multiplier"],
                1.5,
            )

    def test_best_case_prefers_lowest_mae_then_highest_ssim(self) -> None:
        cases = [
            {"index": 1, "value": 0.8, "comparison": {"mae": 2.0, "ssim": 0.95}},
            {"index": 2, "value": 1.0, "comparison": {"mae": 1.0, "ssim": 0.90}},
            {"index": 3, "value": 1.2, "comparison": {"mae": 1.0, "ssim": 0.96}},
            {"index": 4, "value": 1.4},
        ]
        self.assertEqual(select_best_case(cases)["index"], 3)


if __name__ == "__main__":
    unittest.main()
