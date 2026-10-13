"""Small methodology checks, with no matcher execution or optional dependencies."""

import math
import unittest

from benchmark_valentine import markdown, native_identity, side_order, summary, synthetic


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

    def test_three_way_order_rotates_every_runtime_through_each_position(self):
        orders = [side_order(True, number) for number in range(3)]
        self.assertEqual(orders[0], ("rust_before", "rust", "python"))
        for position in range(3):
            self.assertEqual({order[position] for order in orders}, {"rust_before", "rust", "python"})
        self.assertEqual(side_order(True, 3), orders[0])
        self.assertEqual(side_order(False, 0), ("rust", "python"))
        self.assertEqual(side_order(False, 1), ("python", "rust"))
        self.assertEqual(side_order(True, 0, native_only=True), ("rust_before", "rust"))
        self.assertEqual(side_order(True, 1, native_only=True), ("rust", "rust_before"))

    def test_native_identity_requires_an_exact_output_fingerprint(self):
        self.assertEqual(native_identity({"output_sha256": "a" * 64}), "a" * 64)
        for value in (None, "", "a" * 63, "g" * 64):
            with self.assertRaises(ValueError):
                native_identity({"output_sha256": value})
        with self.assertRaises(KeyError):
            native_identity({"matches": []})

    def test_three_way_failures_withhold_both_speedups(self):
        report = {"settings": {"baseline_revision": "e82bdc5"}, "comparisons": [
            {"case": "test", "algorithm": "jaccard", "status": "non-equivalent", "speedup": None,
             "native_speedup": None}]}
        text = markdown(report)
        self.assertIn("Rust before", text)
        self.assertIn("Rust after", text)
        self.assertIn("Python ms", text)
        self.assertEqual(text.count("withheld"), 2)


if __name__ == "__main__":
    unittest.main()
