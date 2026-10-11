"""Small methodology checks, with no matcher execution or optional dependencies."""

import math
import unittest

from benchmark_valentine import markdown, summary, synthetic


class BenchmarkTests(unittest.TestCase):
    def test_inputs_are_deterministic_and_shared_without_labels(self):
        case = synthetic("test", 4, 8, 4)
        self.assertEqual(case, synthetic("test", 4, 8, 4))
        self.assertNotIn("expected", case)
        for source, target in zip(case["source"]["fields"], case["target"]["fields"]):
            self.assertEqual(len(source["samples"]), 8)
            self.assertEqual(len(set(source["samples"])), 4)
            self.assertEqual(len(set(source["samples"]) & set(target["samples"])), 3)

    def test_summary_retains_variation_and_rejects_invalid_measurements(self):
        self.assertEqual(summary([9, 1, 2]), {"median": 2, "min": 1, "max": 9})
        for values in ([], [-1], [math.inf], [math.nan]):
            with self.assertRaises(ValueError):
                summary(values)

    def test_failed_comparisons_do_not_claim_speedups(self):
        report = {"comparisons": [{"case": "test", "algorithm": "cupid", "status": "FAILED / non-equivalent",
                                   "speedup": None}]}
        text = markdown(report)
        self.assertIn("withheld", text)
        self.assertIn("unavailable", text)
        self.assertIn("FAILED / non-equivalent", text)


if __name__ == "__main__":
    unittest.main()
