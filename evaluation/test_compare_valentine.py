"""Standard-library protocol tests. Never import Valentine or score the corpus."""

import collections
import copy
import io
import json
import pathlib
import socket
import tempfile
import unittest
from unittest import mock

import compare_valentine as comparison


Pair = collections.namedtuple("Pair", "source_table source_column target_table target_column")


def fixture():
    return {"format": "fieldkin-northix-v1", "tables": [
        {"id": "left", "fields": [{"id": "full-source-id", "name": "Original_Name", "samples": ["secret sample", None]}]},
        {"id": "right", "fields": [{"id": "full-target-id", "name": "TargetName", "samples": ["target value"]}]}],
        "pairs": [{"id": "pair", "source_table": "left", "target_table": "right"}],
        "labels": {"should_never_be_used": "class"}}


class InputTests(unittest.TestCase):
    def test_matching_inputs_ignore_labels_and_metadata(self):
        original = fixture()
        expected = comparison.comparison_inputs(original, expected_pairs=1)
        changed = copy.deepcopy(original)
        changed["labels"] = object()
        changed["tables"][0]["class_label"] = object()
        changed["tables"][0]["fields"][0]["class"] = object()
        self.assertEqual(expected, comparison.comparison_inputs(changed, expected_pairs=1))
        self.assertEqual(set(expected[0]["left"]["fields"][0]), {"id", "name", "samples"})

    def test_modes_keep_object_inputs_and_pad_without_resampling(self):
        tables, _ = comparison.comparison_inputs(fixture(), expected_pairs=1)
        actual = comparison.columns(tables["left"], "schema_and_samples")["Original_Name"]
        self.assertEqual(len(actual), 64)
        self.assertEqual(actual[:2], ["secret sample", None])
        self.assertTrue(all(value is None for value in actual[2:]))
        self.assertEqual(comparison.columns(tables["left"], "schema_only")["Original_Name"], [None] * 64)

    def test_missing_extra_or_duplicate_pair_inventory_is_rejected(self):
        with self.assertRaises(comparison.VerificationError):
            comparison.comparison_inputs(fixture())
        changed = fixture()
        changed["pairs"] *= 2
        with self.assertRaises(comparison.VerificationError):
            comparison.comparison_inputs(changed, expected_pairs=2)

    def test_duplicate_names_and_ids_are_rejected(self):
        for key in ["id", "name"]:
            changed = fixture()
            field = dict(changed["tables"][0]["fields"][0], id="second", name="second")
            field[key] = changed["tables"][0]["fields"][0][key]
            changed["tables"][0]["fields"].append(field)
            with self.assertRaises(comparison.VerificationError):
                comparison.comparison_inputs(changed, expected_pairs=1)

    def test_oversized_or_numeric_samples_are_rejected_without_values(self):
        for values in [["private"] * 65, [42], [True]]:
            changed = fixture()
            changed["tables"][0]["fields"][0]["samples"] = values
            with self.assertRaises(comparison.VerificationError) as result:
                comparison.comparison_inputs(changed, expected_pairs=1)
            self.assertNotIn("private", str(result.exception))


class ScoreTests(unittest.TestCase):
    def setUp(self):
        self.tables, _ = comparison.comparison_inputs(fixture(), expected_pairs=1)
        self.pair = Pair("aaa", "Original_Name", "bbb", "TargetName")

    def translate(self, matches):
        return comparison.translate_scores(matches, self.tables["left"], self.tables["right"])

    def test_zero_scores_are_retained_and_full_ids_are_restored(self):
        self.assertEqual(self.translate({self.pair: 0.0}),
                         [{"source": "full-source-id", "target": "full-target-id", "score": 0.0}])
        self.assertEqual(self.translate({}), [])

    def test_nonfinite_out_of_range_or_nonnumeric_scores_are_rejected(self):
        for value in [float("nan"), float("inf"), -0.1, 1.1, True, "0.8"]:
            with self.subTest(value=value), self.assertRaises(comparison.VerificationError):
                self.translate({self.pair: value})

    def test_unknown_identity_and_reverse_direction_are_rejected(self):
        for pair in [self.pair._replace(source_column="unknown"), self.pair._replace(source_table="bbb")]:
            with self.assertRaises(comparison.VerificationError):
                self.translate({pair: 0.8})


class GuardTests(unittest.TestCase):
    def test_offline_guard_blocks_connections_and_restores_functions(self):
        original = socket.create_connection
        with comparison.offline_guard():
            with self.assertRaises(comparison.VerificationError):
                socket.create_connection(("unused.example", 443))
        self.assertIs(socket.create_connection, original)

    def test_quiet_discards_output(self):
        output = io.StringIO()
        with mock.patch("sys.stdout", output), comparison.quiet():
            print("private sample")
        self.assertEqual(output.getvalue(), "")

    def test_duplicate_json_keys_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = pathlib.Path(temporary) / "record.json"
            path.write_text('{"a":1,"a":2}', encoding="utf8")
            with self.assertRaises(comparison.VerificationError):
                comparison.load(path)

    def test_write_new_preserves_existing_artifact(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = pathlib.Path(temporary) / "result.json"
            comparison.write_new(path, {"old": True})
            with self.assertRaises(FileExistsError):
                comparison.write_new(path, {"new": True})
            self.assertEqual(comparison.load(path), {"old": True})

    def test_prepare_rejects_existing_directory_before_context(self):
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(comparison, "context") as context:
            with self.assertRaises(comparison.VerificationError):
                comparison.prepare(pathlib.Path(temporary), pathlib.Path(temporary))
            context.assert_not_called()

    def test_run_rejects_partial_output_before_scoring(self):
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(comparison, "score_modes") as scorer:
            path = pathlib.Path(temporary)
            comparison.write_new(path / "prepare.json", {})
            comparison.write_new(path / "valentine-schema_only.json", {})
            with self.assertRaises(comparison.VerificationError):
                comparison.run(path, path)
            scorer.assert_not_called()

    def run_fixture(self, temporary, contexts, scorer):
        path = pathlib.Path(temporary)
        before = {"fixture": "unchanged"}
        comparison.write_new(path / "prepare.json", {"protocol": comparison.PROTOCOL,
                             "settings": comparison.SETTINGS, "modes": list(comparison.MODES),
                             "context": before, "context_sha256": comparison.canonical_hash(before)})
        tables, pairs = comparison.comparison_inputs(fixture(), expected_pairs=1)
        original_load = comparison.load
        def load(input_path):
            return original_load(input_path) if input_path.name == "prepare.json" else fixture()
        with mock.patch.object(comparison, "context", side_effect=contexts), \
                mock.patch.object(comparison, "load", side_effect=load), \
                mock.patch.object(comparison, "comparison_inputs", return_value=(tables, pairs)), \
                mock.patch.object(comparison, "score_modes", side_effect=scorer):
            comparison.run(path, path)

    def test_changed_prepared_context_rejects_before_scoring(self):
        with tempfile.TemporaryDirectory() as temporary:
            scorer = mock.Mock()
            with self.assertRaises(comparison.VerificationError):
                self.run_fixture(temporary, [{"fixture": "changed"}], scorer)
            scorer.assert_not_called()
            self.assertFalse((pathlib.Path(temporary) / "run.json").exists())

    def test_post_run_drift_or_incomplete_artifacts_never_get_success(self):
        for drift in [False, True]:
            with tempfile.TemporaryDirectory() as temporary:
                def scorer(*args):
                    comparison.write_new(args[-1] / comparison.ARTIFACTS[comparison.MODES[0]], {})
                contexts = [{"fixture": "unchanged"}, {"fixture": "changed" if drift else "unchanged"}]
                with self.assertRaises((comparison.VerificationError, FileNotFoundError)):
                    self.run_fixture(temporary, contexts, scorer)
                self.assertFalse((pathlib.Path(temporary) / "run.json").exists())

    def test_success_requires_both_artifacts_and_matching_context(self):
        with tempfile.TemporaryDirectory() as temporary:
            def scorer(*args):
                for name in comparison.ARTIFACTS.values():
                    comparison.write_new(args[-1] / name, {})
            self.run_fixture(temporary, [{"fixture": "unchanged"}] * 2, scorer)
            record = comparison.load(pathlib.Path(temporary) / "run.json")
            self.assertEqual(record["status"], "complete")
            self.assertEqual(set(record["artifacts_sha256"]), set(comparison.ARTIFACTS.values()))


if __name__ == "__main__":
    unittest.main()
