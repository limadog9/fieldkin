"""Small validation-tool checks; no timing assertions or matcher execution."""

import copy
import unittest

from benchmark_performance import assignment_identity, cases, rectangular_algorithm


class AssignmentComparisonTests(unittest.TestCase):
    def report(self, objective="score-before-filter", count=None):
        return {"result_count": 2, "assignment_validation": {
            "input_sha256": "a" * 64, "selected_sha256": "b" * 64,
            "objective": objective, "eligible_pair_count": count, "score_units": 8192,
        }}

    def test_opt_in_mapping_rejects_unrelated_operations(self):
        self.assertEqual(rectangular_algorithm("hungarian-aware-guarded"), "rectangular-aware-guarded")
        with self.assertRaises(ValueError):
            rectangular_algorithm("metrics")

    def test_equal_objective_can_have_different_selected_pairs(self):
        before = self.report()
        after = copy.deepcopy(before)
        after["assignment_validation"]["selected_sha256"] = "c" * 64
        self.assertEqual(assignment_identity(before), assignment_identity(after))
        for key, value in (("input_sha256", "d" * 64), ("score_units", 8191)):
            changed = copy.deepcopy(after)
            changed["assignment_validation"][key] = value
            self.assertNotEqual(assignment_identity(before), assignment_identity(changed))

    def test_cardinality_is_part_of_threshold_aware_objective(self):
        before = self.report("eligible-cardinality-then-score", 2)
        after = copy.deepcopy(before)
        after["result_count"] = 1
        after["assignment_validation"]["eligible_pair_count"] = 1
        self.assertNotEqual(assignment_identity(before), assignment_identity(after))

    def test_missing_invalid_and_postfilter_validation_is_rejected(self):
        with self.assertRaises(KeyError):
            assignment_identity({})
        for key, value in (("score_units", -1), ("score_units", 1.5),
                           ("input_sha256", "bad"), ("objective", "filtered-score"),
                           ("eligible_pair_count", 2)):
            invalid = self.report()
            invalid["assignment_validation"][key] = value
            with self.assertRaises(ValueError):
                assignment_identity(invalid)
        invalid = self.report("eligible-cardinality-then-score", 1)
        with self.assertRaises(ValueError):
            assignment_identity(invalid)

    def test_requested_dimensions_and_controls_are_available(self):
        workloads = cases()
        for objective in ("hungarian", "hungarian-aware"):
            for size in ("512x1", "1x512", "1024x8", "8x1024", "4096x16", "16x4096",
                         "10000x1", "1x10000", "64x64", "128x128"):
                self.assertIn(f"{objective}-varied{size}", workloads)
            for control in ("tied", "sparse", "guarded"):
                self.assertIn(f"{objective}-{control}1024x8", workloads)


if __name__ == "__main__":
    unittest.main()
