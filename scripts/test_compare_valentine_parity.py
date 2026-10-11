"""Dependency-free checks for the live parity comparator's ranking semantics."""

import unittest

from compare_valentine_parity import compare_case, ranking_differences


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


if __name__ == "__main__":
    unittest.main()
