"""Dependency-free checks for the live parity comparator's ranking semantics."""

import copy
import unittest

from compare_valentine_parity import compare_case, ranking_differences, upstream_diagnostic


def scores(*values):
    return {("source", "field", "target", chr(97 + index)): value
            for index, value in enumerate(values)}


class RankingTests(unittest.TestCase):
    def test_empty_equal_and_lexical_ties(self):
        self.assertEqual(ranking_differences({}, {}, 1e-6), [])
        original = scores(0.9, 0.9, 0.1)
        self.assertEqual(ranking_differences(original, dict(reversed(list(original.items()))), 1e-6), [])

    def test_rounding_tie_is_not_a_material_ranking_change(self):
        changes = ranking_differences(scores(0.5, 0.5 + 5e-7), scores(0.5 + 5e-7, 0.5), 1e-6)
        self.assertEqual(len(changes), 1)
        self.assertTrue(changes[0]["equivalent"])
        self.assertTrue(changes[0]["inversions"][0]["rounding_tie"])

    def test_material_reversal_and_missing_target_fail(self):
        changes = ranking_differences(scores(0.9, 0.1), scores(0.1, 0.9), 1e-6)
        self.assertFalse(changes[0]["equivalent"])
        self.assertFalse(changes[0]["inversions"][0]["rounding_tie"])
        self.assertFalse(ranking_differences(scores(0.9), {}, 1e-6)[0]["equivalent"])

    def test_scores_across_different_sources_do_not_define_a_ranking(self):
        first = ("source", "first", "target", "a")
        second = ("source", "second", "target", "a")
        self.assertEqual(ranking_differences({first: 0.9, second: 0.1}, {first: 0.1, second: 0.9}, 1e-6), [])

    def test_ranking_ties_do_not_relax_score_tolerance(self):
        def result(values):
            return {"algorithms": {"coma": {"error": None, "matches": [
                {"pair": dict(zip(("source_table", "source_column", "target_table", "target_column"), key)),
                 "score": value} for key, value in values.items()]}}}

        compared = compare_case("test", "coma", result(scores(0.5, 0.5)),
                                result(scores(0.6, 0.6)),
                                {"source_column_names": ["field"], "target_column_names": ["a", "b"],
                                 "verified": True}, True)
        self.assertTrue(compared["exact_rankings"])
        self.assertFalse(compared["within_tolerance_scores"])
        self.assertFalse(compared["equivalent"])


class UpstreamDiagnosticTests(unittest.TestCase):
    def report(self, path="eval/customers.json", algorithm="cupid", rust=None, python=None):
        def result(values):
            return {"algorithms": {algorithm: {"error": None, "matches": [
                {"pair": dict(zip(("source_table", "source_column", "target_table", "target_column"), pair)),
                 "score": score} for pair, score in values]}}}

        default = [(('source', 'balance', 'target', 'account_balance'), 0.7767211111111111)]
        case = compare_case(path, algorithm, result(rust or default), result(python or default),
                            {"source_column_names": ["balance", "other", "depth", "latitude", "longitude"],
                             "target_column_names": ["account_balance", "other"]
                             + [f"geometry.coordinates[{index}]" for index in range(3)],
                             "verified": True}, True)
        return {"metadata": {"context_verified": True, "reference_provenance": {"mode": "upstream"}},
                "coverage": {"complete": True, "expected_case_count": 1}, "cases": [case]}

    def test_exact_and_known_cupid_difference_preserve_raw_failure(self):
        self.assertTrue(upstream_diagnostic(self.report())["passed"])
        report = self.report(python=[(('source', 'balance', 'target', 'account_balance'), 0.7630458974358975)])
        before = copy.deepcopy(report)
        self.assertTrue(upstream_diagnostic(report)["passed"])
        self.assertEqual(report, before)
        self.assertFalse(report["cases"][0]["equivalent"])

    def test_cupid_allowlist_does_not_hide_other_scores_pairs_or_cases(self):
        pair = ('source', 'other', 'target', 'other')
        for report in (
                self.report(rust=[(pair, 0.8)], python=[(pair, 0.7)]),
                self.report(python=[(pair, 0.8)]),
                self.report(python=[(('source', 'balance', 'target', 'account_balance'), 0.7)]),
                self.report(path="eval/another.json",
                            python=[(('source', 'balance', 'target', 'account_balance'), 0.7)])):
            self.assertFalse(upstream_diagnostic(report)["passed"])

    def test_known_distribution_sanitization_only(self):
        pairs = [(('source', source, 'target', f'geometry.coordinates[{index}]'), 1.0)
                 for source, index in (('depth', 2), ('latitude', 1), ('longitude', 0))]
        sanitized = [(tuple(value.replace('[', '_').replace(']', '_') for value in pair), score)
                     for pair, score in pairs]
        report = self.report("eval_realworld/usgs_earthquakes.json", "distribution", pairs, sanitized)
        self.assertTrue(upstream_diagnostic(report)["passed"])
        changed = copy.deepcopy(report)
        changed["cases"][0]["unshared_scores"]["python"][0]["score"] = 0.9
        self.assertFalse(upstream_diagnostic(changed)["passed"])
        report["cases"][0]["extra_in_rust"].pop()
        self.assertFalse(upstream_diagnostic(report)["passed"])

    def test_setup_errors_incomplete_empty_and_wrong_reference_fail(self):
        for change in ("context", "coverage", "empty", "count", "reference", "error"):
            report = self.report()
            if change == "context":
                report["metadata"]["context_verified"] = False
            elif change == "coverage":
                report["coverage"]["complete"] = False
            elif change == "empty":
                report["cases"] = []
            elif change == "count":
                report["coverage"]["expected_case_count"] = 2
            elif change == "reference":
                report["metadata"]["reference_provenance"]["mode"] = "fixed"
            else:
                report["cases"][0].update(equivalent=False, both_succeeded=False)
            self.assertFalse(upstream_diagnostic(report)["passed"], change)


if __name__ == "__main__":
    unittest.main()
