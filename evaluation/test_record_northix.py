"""Northix execution guards; fake tools only, no compiler or matcher execution."""

import io
import json
import pathlib
import subprocess
import unittest
from unittest import mock

import record_northix
import test_verified
import verified


class RecordedNorthixTests(unittest.TestCase):
    def setUp(self):
        # Reuse the established isolated repository/fake compiler scaffolding,
        # without inheriting or rerunning its 24 independent guard tests.
        self.fixture = test_verified.VerifiedEvaluationTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.fixture.execute_mock.side_effect = self.fake_execute
        self.mutation = None
        self.missing = False
        self.extra = False
        self.fail = False
        self.score_paths = []

    def fake_execute(self, command, cwd, env=None):
        del env
        self.assertEqual(cwd, self.fixture.root)
        self.assertEqual(command[1:3], ["--northix", "--output"])
        self.assertEqual(command[4:], [part for path in self.score_paths for part in ["--scores", str(path)]])
        directory = pathlib.Path(command[3])
        self.assertFalse(directory.exists(), "the runner must not precreate the artifacts child")
        directory.mkdir()
        for name in record_northix.ARTIFACTS:
            if self.missing and name == "predictions.jsonl":
                continue
            (directory / name).write_bytes(b"synthetic recorded output\n")
        if self.extra:
            (directory / "unrequested.json").write_bytes(b"unexpected output")
        if self.mutation:
            self.mutation()
        if self.fail:
            raise subprocess.CalledProcessError(1, command)

    def run_record(self):
        return record_northix.run(self.fixture.build_dir, self.fixture.output, self.score_paths, root=self.fixture.root)

    def make_scores(self):
        for mode in ["schema_only", "schema_and_samples"]:
            path = self.fixture.root / f"{mode}.json"
            # Semantic score validation belongs to the Rust evaluator. These
            # files exercise only the wrapper's fixed-argument and byte guards.
            path.write_text(json.dumps({"mode": mode}), encoding="utf8")
            self.score_paths.append(path)

    def test_zero_scores_uses_only_fixed_command_and_seals_complete_artifacts(self):
        self.fixture.build()
        result = self.run_record()
        self.assertEqual(result["protocol"], record_northix.PROTOCOL)
        self.assertEqual(result["score_files"], [])
        self.assertEqual(result["partition"], "fixed-external-diagnostic")
        self.assertFalse(result["reserved_holdouts_scored"])
        self.assertEqual(set(result["artifact_sha256"]), set(record_northix.ARTIFACTS))
        self.assertEqual(verified._load(self.fixture.output / "run.json"), result)
        for name, digest in result["artifact_sha256"].items():
            self.assertEqual(digest, verified.raw_sha256(self.fixture.output / "artifacts" / name))

    def test_two_scores_are_bound_to_exact_input_bytes_and_fixed_arguments(self):
        self.fixture.build()
        self.make_scores()
        result = self.run_record()
        for path, record in zip(self.score_paths, result["score_files"]):
            self.assertEqual(record, {"path": str(path), "bytes": path.stat().st_size, "sha256": verified.raw_sha256(path)})
        self.assertEqual(result["build_record_sha256"], verified.raw_sha256(self.fixture.build_dir / "build.json"))

    def test_score_counts_duplicates_and_oversize_reject_before_execution(self):
        self.fixture.build()
        self.make_scores()
        for scores in [self.score_paths[:1], self.score_paths + self.score_paths[:1], [self.score_paths[0]] * 2]:
            with self.assertRaises(ValueError):
                record_northix.run(self.fixture.build_dir, self.fixture.output, scores, root=self.fixture.root)
            self.fixture.execute_mock.assert_not_called()
            self.fixture.assert_no_success()
        with mock.patch.object(record_northix, "MAX_SCORE_BYTES", 1), self.assertRaises(ValueError):
            self.run_record()
        self.fixture.execute_mock.assert_not_called()

    def test_missing_or_symlink_score_input_is_rejected(self):
        self.fixture.build()
        self.make_scores()
        with mock.patch.object(pathlib.Path, "is_symlink", return_value=True), self.assertRaises(ValueError):
            self.run_record()
        self.score_paths[0].unlink()
        with self.assertRaises(OSError):
            self.run_record()
        self.fixture.execute_mock.assert_not_called()

    def test_score_drift_during_execution_never_produces_success(self):
        self.fixture.build()
        self.make_scores()
        self.mutation = lambda: self.score_paths[0].write_bytes(b"changed scores")
        with self.assertRaises(ValueError):
            self.run_record()
        self.fixture.assert_no_success()

    def test_score_drift_while_inspecting_artifacts_never_produces_success(self):
        self.fixture.build()
        self.make_scores()
        original = record_northix.artifact_hashes
        def drifting(output):
            result = original(output)
            self.score_paths[1].write_bytes(b"late change")
            return result
        with mock.patch.object(record_northix, "artifact_hashes", side_effect=drifting), self.assertRaises(ValueError):
            self.run_record()
        self.fixture.assert_no_success()

    def test_current_northix_input_drift_rejects_before_execution(self):
        self.fixture.build()
        for relative in ["evaluation/northix-protocol.json", "evaluation/fixtures/northix-v1.json", "evaluation/record_northix.py"]:
            path = self.fixture.root / relative
            original = path.read_bytes()
            try:
                path.write_bytes(original + b"\nchanged")
                with self.assertRaises(ValueError):
                    self.run_record()
                self.fixture.execute_mock.assert_not_called()
                self.fixture.assert_no_success()
            finally:
                path.write_bytes(original)
                self.fixture.new_output()

    def test_source_or_binary_or_record_drift_during_execution_is_rejected(self):
        self.fixture.build()
        paths = [self.fixture.root / "evaluation/import_northix.py", self.fixture.build_dir / "build.json"]
        binary = self.fixture.build_dir / verified._load(self.fixture.build_dir / "build.json")["binary_path"]
        paths.append(binary)
        for path in paths:
            original = path.read_bytes()
            try:
                self.mutation = lambda path=path: path.write_bytes(original + b"\nchanged")
                with self.assertRaises(ValueError):
                    self.run_record()
                self.fixture.assert_no_success()
            finally:
                path.write_bytes(original)
                self.fixture.new_output()

    def test_missing_extra_and_oversize_outputs_never_produce_success(self):
        self.fixture.build()
        for missing, extra in [(True, False), (False, True)]:
            self.missing, self.extra = missing, extra
            with self.assertRaises(ValueError):
                self.run_record()
            self.fixture.assert_no_success()
            self.fixture.new_output()
        self.missing = self.extra = False
        with mock.patch.object(verified, "MAX_ARTIFACT_BYTES", 1), self.assertRaises(ValueError):
            self.run_record()
        self.fixture.assert_no_success()

    def test_failed_execution_retains_partial_outputs_and_cannot_reuse_destination(self):
        self.fixture.build()
        self.fail = True
        with self.assertRaises(subprocess.CalledProcessError):
            self.run_record()
        self.fixture.assert_no_success()
        self.assertTrue((self.fixture.output / "artifacts" / "results.json").exists())
        self.fail = False
        calls = self.fixture.execute_mock.call_count
        with self.assertRaises(ValueError):
            self.run_record()
        self.assertEqual(self.fixture.execute_mock.call_count, calls)

    def test_outside_or_overlapping_destinations_are_rejected(self):
        self.fixture.build()
        for output in [self.fixture.build_dir, self.fixture.build_dir / "nested", self.fixture.root.parent / "outside", self.fixture.root / "target"]:
            with self.assertRaises(ValueError):
                record_northix.run(self.fixture.build_dir, output, root=self.fixture.root)
        self.fixture.execute_mock.assert_not_called()

    def test_cli_has_no_arbitrary_evaluator_or_holdout_switches(self):
        args = ["--build-dir", str(self.fixture.build_dir), "--output", str(self.fixture.output)]
        for flag in ["--holdout", "--acknowledge-holdout", "--external", "--threshold", "--arbitrary"]:
            with mock.patch("sys.stderr", new_callable=io.StringIO), self.assertRaises(SystemExit) as failure:
                record_northix.main(args + [flag])
            self.assertEqual(failure.exception.code, 2)
        self.fixture.execute_mock.assert_not_called()


if __name__ == "__main__":
    unittest.main()
