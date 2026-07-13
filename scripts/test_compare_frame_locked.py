#!/usr/bin/env python3
"""Synthetic self-test for compare_frame_locked.py."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import numpy as np
from PIL import Image

if __package__:
    from scripts.compare_frame_locked import (
        ComparisonError,
        parse_frame_specs,
        run_comparison,
    )
else:
    from compare_frame_locked import ComparisonError, parse_frame_specs, run_comparison


class FrameLockedComparisonSelfTest(unittest.TestCase):
    def test_synthetic_exact_and_changed_frames(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            rust_dir = root / "rust"
            ae_dir = root / "ae"
            out_dir = root / "out"
            rust_dir.mkdir()
            ae_dir.mkdir()

            identical = np.zeros((2, 4, 4), dtype=np.uint8)
            identical[:, :, :] = (20, 40, 60, 255)
            changed = identical.copy()
            changed[0, 1, 0] += 10
            Image.fromarray(identical).save(rust_dir / "frame_000001.png")
            Image.fromarray(identical).save(ae_dir / "ae_00001.png")
            Image.fromarray(identical).save(rust_dir / "frame_000002.png")
            Image.fromarray(changed).save(ae_dir / "ae_00002.png")

            metrics_path = run_comparison(
                rust_dir=rust_dir,
                ae_dir=ae_dir,
                frame_specs=["1-2,2"],
                out_dir=out_dir,
            )
            first_metrics_bytes = metrics_path.read_bytes()
            report = json.loads(metrics_path.read_text(encoding="utf-8"))

            self.assertEqual(report["alignment"]["requested_frames"], [1, 2])
            self.assertEqual(report["alignment"]["frame_offset"], 0)
            self.assertFalse(report["alignment"]["nearest_frame_matching"])
            self.assertEqual(report["summary"]["frame_count"], 2)
            self.assertEqual(report["summary"]["identical_frame_count"], 1)

            exact_metrics = report["frames"][0]["metrics"]
            self.assertTrue(exact_metrics["identical"])
            self.assertIsNone(exact_metrics["bbox"])
            self.assertEqual(exact_metrics["mae"], 0.0)
            self.assertIsNone(exact_metrics["psnr_db"])
            self.assertTrue(exact_metrics["psnr_is_infinite"])
            self.assertEqual(exact_metrics["ssim"], 1.0)

            changed_metrics = report["frames"][1]["metrics"]
            self.assertFalse(changed_metrics["identical"])
            self.assertEqual(changed_metrics["bbox"], [1, 0, 2, 1])
            self.assertAlmostEqual(changed_metrics["mae"], 0.3125)
            self.assertEqual(changed_metrics["max_abs_diff"], 10)
            self.assertLess(changed_metrics["ssim"], 1.0)

            side_path = out_dir / report["frames"][1]["artifacts"]["side_by_side"]
            diff_path = out_dir / report["frames"][1]["artifacts"]["abs_diff"]
            with Image.open(side_path) as side:
                self.assertEqual(side.size, (8, 2))
                self.assertEqual(side.getpixel((1, 0)), (20, 40, 60, 255))
                self.assertEqual(side.getpixel((5, 0)), (30, 40, 60, 255))
            with Image.open(diff_path) as diff:
                self.assertEqual(diff.mode, "RGB")
                self.assertEqual(diff.getpixel((1, 0)), (10, 0, 0))

            first_side_bytes = side_path.read_bytes()
            first_diff_bytes = diff_path.read_bytes()
            run_comparison(
                rust_dir=rust_dir,
                ae_dir=ae_dir,
                frame_specs=["1-2"],
                out_dir=out_dir,
            )
            self.assertEqual(metrics_path.read_bytes(), first_metrics_bytes)
            self.assertEqual(side_path.read_bytes(), first_side_bytes)
            self.assertEqual(diff_path.read_bytes(), first_diff_bytes)

    def test_missing_same_frame_never_uses_adjacent_frame(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            rust_dir = root / "rust"
            ae_dir = root / "ae"
            rust_dir.mkdir()
            ae_dir.mkdir()
            image = Image.new("RGBA", (2, 2), (1, 2, 3, 255))
            image.save(rust_dir / "render-000010.png")
            image.save(ae_dir / "render-000009.png")
            image.save(ae_dir / "render-000011.png")

            with self.assertRaisesRegex(ComparisonError, "AE frame number.*10"):
                run_comparison(
                    rust_dir=rust_dir,
                    ae_dir=ae_dir,
                    frame_specs=["10"],
                    out_dir=root / "out",
                )

    def test_frame_spec_is_inclusive_sorted_and_unique(self) -> None:
        self.assertEqual(parse_frame_specs(["7,3-5", "4", "-2--1"]), [-2, -1, 3, 4, 5, 7])


if __name__ == "__main__":
    unittest.main()
