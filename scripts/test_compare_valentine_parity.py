"""Dependency-free checks for live parity validation and ranking semantics."""

import copy
from contextlib import redirect_stderr, redirect_stdout
import hashlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import compare_valentine_parity as comparator
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


class CompleteComparisonTests(unittest.TestCase):
    """Exercise the complete comparator against small, real filesystem fixtures."""

    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="fieldkin-parity-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        root_patch = patch.object(comparator, "ROOT", self.root)
        root_patch.start()
        self.addCleanup(root_patch.stop)
        source = self.root / ".upstream/valentine"
        (source / "valentine").mkdir(parents=True)
        (source / "valentine/__init__.py").write_text("# Comparator provenance fixture.\n")
        source_hash = comparator.package_sha256(source)
        dataset = self.root / "eval/fixture.json"
        dataset.parent.mkdir()
        dataset.write_text(json.dumps({"source": {"fields": [{"name": "field"}]},
                                       "target": {"fields": [{"name": "a"}]}}))
        match = {"pair": comparator.pair_dict(("source", "field", "target", "a")), "score": 0.5}
        self.rust = {
            "upstream_revision": comparator.EXPECTED_UPSTREAM_REVISION,
            "dtype_mapping": {"text": "varchar"},
            "datasets": [{"path": "eval/fixture.json", "name": "fixture",
                          "input_sha256": hashlib.sha256(dataset.read_bytes()).hexdigest(),
                          "source_columns": ["field"], "target_columns": ["a"],
                          "algorithms": {algorithm: {"error": None, "matches": [copy.deepcopy(match)]}
                                         for algorithm in comparator.ALGORITHMS}}],
        }
        self.python = copy.deepcopy(self.rust)
        self.python["reference"] = {
            "mode": "upstream", "upstream_revision": comparator.EXPECTED_UPSTREAM_REVISION,
            "source_root": ".upstream/valentine", "base_source_sha256": source_hash,
            "source_sha256": source_hash, "patches": [],
        }
        self.rust_path = self.root / "rust.json"
        self.python_path = self.root / "python.json"
        self.rust_path.write_text(json.dumps(self.rust))
        self.python_path.write_text(json.dumps(self.python))

    def cli(self, *extra):
        arguments = ["compare_valentine_parity.py", "--rust", str(self.rust_path),
                     "--python", str(self.python_path), "--output", str(self.root / "comparison.json"), *extra]
        errors = io.StringIO()
        with patch.object(sys, "argv", arguments), redirect_stdout(io.StringIO()), redirect_stderr(errors):
            status = comparator.main()
        return status, errors.getvalue()

    def test_complete_reports_pass_and_missing_or_malformed_output_files_fail(self):
        self.assertEqual(self.cli()[0], 0)
        report = json.loads((self.root / "comparison.json").read_text())
        self.assertTrue(report["aggregate"]["all_exact"])
        self.assertEqual(report["aggregate"]["case_count"], len(comparator.ALGORITHMS))
        for path in (self.rust_path, self.python_path):
            original = path.read_text()
            for malformed in (False, True):
                with self.subTest(path=path.name, malformed=malformed):
                    if malformed:
                        path.write_text("{")
                    else:
                        path.unlink()
                    status, errors = self.cli()
                    self.assertEqual(status, 2)
                    self.assertIn("Cannot compare matcher results", errors)
                    path.write_text(original)

    def test_missing_datasets_algorithms_and_result_fields_fail(self):
        for side in ("rust", "python"):
            for missing in ("dataset", "algorithm", "matches", "error"):
                with self.subTest(side=side, missing=missing):
                    documents = {"rust": copy.deepcopy(self.rust), "python": copy.deepcopy(self.python)}
                    document = documents[side]
                    if missing == "dataset":
                        document["datasets"] = []
                    elif missing == "algorithm":
                        del document["datasets"][0]["algorithms"]["jaccard"]
                    else:
                        del document["datasets"][0]["algorithms"]["jaccard"][missing]
                    report = comparator.compare(documents["rust"], documents["python"])
                    self.assertFalse(report["aggregate"]["all_equivalent"])
                    self.assertLess(report["aggregate"]["successful_case_count"], len(comparator.ALGORITHMS))

    def test_duplicate_dataset_and_correspondence_identifiers_fail(self):
        for side in ("rust", "python"):
            documents = {"rust": copy.deepcopy(self.rust), "python": copy.deepcopy(self.python)}
            documents[side]["datasets"].append(copy.deepcopy(documents[side]["datasets"][0]))
            with self.assertRaisesRegex(ValueError, "duplicate dataset path"):
                comparator.compare(documents["rust"], documents["python"])
        rust, python = copy.deepcopy(self.rust), copy.deepcopy(self.python)
        for document in (rust, python):
            matches = document["datasets"][0]["algorithms"]["jaccard"]["matches"]
            matches.append(copy.deepcopy(matches[0]))
        report = comparator.compare(rust, python)
        case = next(case for case in report["cases"] if case["algorithm"] == "jaccard")
        self.assertTrue(case["pair_sets_equal"])
        self.assertFalse(report["aggregate"]["all_equivalent"])
        self.assertTrue(any("duplicate correspondence" in issue for issue in case["schema_issues"]))

    def test_equal_but_invalid_correspondence_identifiers_fail(self):
        for field in comparator.PAIR_FIELDS:
            with self.subTest(field=field):
                rust, python = copy.deepcopy(self.rust), copy.deepcopy(self.python)
                for document in (rust, python):
                    document["datasets"][0]["algorithms"]["jaccard"]["matches"][0]["pair"][field] = "absent"
                report = comparator.compare(rust, python)
                case = next(case for case in report["cases"] if case["algorithm"] == "jaccard")
                self.assertTrue(case["pair_sets_equal"])
                self.assertFalse(case["identifiers_valid"])
                self.assertFalse(report["aggregate"]["all_equivalent"])

    def test_corrupted_recorded_provenance_and_changed_source_fail(self):
        for field, value in (("upstream_revision", "wrong"), ("base_source_sha256", "0" * 64),
                             ("source_sha256", "0" * 64), ("source_root", "../outside"),
                             ("patches", [{"path": "unexpected.patch", "sha256": "0" * 64}])):
            with self.subTest(field=field):
                python = copy.deepcopy(self.python)
                python["reference"][field] = value
                report = comparator.compare(self.rust, python)
                self.assertFalse(report["metadata"]["reference_provenance_verified"])
                self.assertFalse(report["aggregate"]["all_equivalent"])
        (self.root / ".upstream/valentine/valentine/__init__.py").write_text("# Changed after hashing.\n")
        self.assertFalse(comparator.compare(self.rust, self.python)["aggregate"]["all_equivalent"])

    def test_score_tolerances_are_enforced_through_complete_comparison(self):
        for algorithm, tolerance in comparator.TOLERANCES.items():
            for factor in (0.5, 2.0):
                with self.subTest(algorithm=algorithm, factor=factor):
                    python = copy.deepcopy(self.python)
                    python["datasets"][0]["algorithms"][algorithm]["matches"][0]["score"] += factor * tolerance
                    report = comparator.compare(self.rust, python)
                    self.assertEqual(report["aggregate"]["all_equivalent"], factor < 1)
                    self.assertFalse(report["aggregate"]["all_exact"])

    def test_corrected_reference_requirement_preserves_legacy_comparison_defaults(self):
        for legacy in (False, True):
            with self.subTest(legacy=legacy):
                python = copy.deepcopy(self.python)
                if legacy:
                    del python["reference"]
                self.python_path.write_text(json.dumps(python))
                self.assertEqual(self.cli()[0], 0)
                status, errors = self.cli("--require-reference-fixes")
                self.assertEqual(status, 2)
                self.assertIn("requires verified fixed-reference provenance", errors)


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
