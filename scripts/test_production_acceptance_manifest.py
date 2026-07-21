#!/usr/bin/env python3

import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ACCEPTANCE = ROOT / "fixtures/bot_corpus/production_acceptance_20260717.json"
GOLDENS = ROOT / "fixtures/bot_corpus/production_goldens.p0p1.json"


class ProductionAcceptanceManifestTest(unittest.TestCase):
    def test_acceptance_evidence_is_complete_and_unique(self) -> None:
        acceptance = json.loads(ACCEPTANCE.read_text(encoding="utf-8"))
        validation = acceptance["validation"]
        cases = acceptance["cases"]

        self.assertEqual(acceptance["schema"], "ae-native-renderer.production-acceptance.v1")
        self.assertEqual(validation["total"], 34)
        self.assertEqual(validation["complete"], 34)
        self.assertEqual(validation["incomplete"], 0)
        self.assertEqual(validation["with_audio_capability_supported"], 34)
        self.assertEqual(validation["not_implemented"], 0)
        self.assertEqual(validation["unsupported"], 0)
        self.assertEqual(len(cases), validation["total"])
        self.assertEqual(len({case["id"] for case in cases}), len(cases))
        self.assertEqual(len({case["job_id"] for case in cases}), len(cases))
        self.assertTrue(all(len(case["job_id"]) == 32 for case in cases))

    def test_goldens_point_at_completed_acceptance(self) -> None:
        goldens = json.loads(GOLDENS.read_text(encoding="utf-8"))
        evidence = ROOT / goldens["acceptance_evidence"]
        self.assertEqual(evidence.resolve(), ACCEPTANCE.resolve())
        self.assertTrue(evidence.is_file())

        by_id = {case["id"]: case for case in goldens["cases"]}
        self.assertEqual(
            by_id["trendy_5th_real_job"]["reference_status"],
            "ae_reference_downloaded_and_compared",
        )
        self.assertEqual(
            by_id["brat_5th_real_job"]["reference_status"],
            "ae_reference_downloaded_and_compared",
        )
        self.assertEqual(
            by_id["bigtest_f1_f5_render_plan_corpus"]["reference_status"],
            "32_ae_exports_downloaded_and_compared",
        )
        self.assertNotIn(
            "full AE reference media for every /bigtest F1-F5 request",
            goldens["known_p1_gaps"],
        )


if __name__ == "__main__":
    unittest.main()
