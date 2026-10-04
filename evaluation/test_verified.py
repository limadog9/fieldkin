"""Verified-evaluation guard tests; no Rust compiler or matcher is executed."""

import hashlib
import io
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import types
import unittest
from unittest import mock

import verified


REPOSITORY = pathlib.Path(__file__).resolve().parent.parent


class VerifiedEvaluationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="fieldkin-verified-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = pathlib.Path(self.temporary.name).resolve() / "repository"
        self.root.mkdir()
        for directory in ["src", "evaluation/src", "evaluation/corpus", "evaluation/external"]:
            shutil.copytree(REPOSITORY / directory, self.root / directory)
        for relative in [
            "Cargo.toml", "Cargo.lock", "README.md", "evaluation/Cargo.toml",
            "evaluation/protocol.json", "evaluation/stage3-protocol.json",
            "evaluation/release-protocol.json", "evaluation/corrective-protocol.json",
            "evaluation/external-protocol.json", "evaluation/import_t2d.py",
            "evaluation/verified.py",
        ]:
            shutil.copyfile(REPOSITORY / relative, self.root / relative)
        self.build_dir = self.root / "target" / "verified-build"
        self.output = self.root / "target" / "verified-results"
        self.capture_patch = mock.patch.object(verified, "capture", side_effect=self.fake_capture)
        self.capture_mock = self.capture_patch.start()
        self.addCleanup(self.capture_patch.stop)
        self.execute_patch = mock.patch.object(verified, "execute", side_effect=self.fake_execute)
        self.execute_mock = self.execute_patch.start()
        self.addCleanup(self.execute_patch.stop)
        self.environment_patch = mock.patch.dict(
            os.environ, {"CARGO_HOME": str(pathlib.Path(self.temporary.name).resolve() / "cargo-home")}
        )
        self.environment_patch.start()
        self.addCleanup(self.environment_patch.stop)
        self.compilation_mutation = None
        self.execution_mutation = None
        self.extra_artifact = False
        self.missing_artifact = False
        self.fail_execution = False
        self.output_number = 0

    def fake_capture(self, command, cwd, env=None):
        del cwd, env
        arguments = [str(argument) for argument in command]
        if arguments[0] == "git":
            return "0" * 40
        if arguments[0] == "rustc":
            return "rustc 1.85.0 (test)\nhost: x86_64-test-host\nrelease: 1.85.0\n"
        if arguments[0] == "cargo" and "build" not in arguments:
            return "cargo 1.85.0 (test)\nrelease: 1.85.0\nhost: x86_64-test-host\n"
        if arguments[0] == "cargo" and "build" in arguments:
            target = pathlib.Path(arguments[arguments.index("--target-dir") + 1])
            binary = target / "release" / ("fieldkin-eval.exe" if os.name == "nt" else "fieldkin-eval")
            binary.parent.mkdir(parents=True, exist_ok=True)
            binary.write_bytes(b"a fake compiled evaluator; never executed")
            if self.compilation_mutation:
                self.compilation_mutation()
            return json.dumps({
                "reason": "compiler-artifact",
                "package_id": "path+file:///test/evaluation#fieldkin-eval@0.1.0",
                "target": {"name": "fieldkin-eval", "kind": ["bin"]},
                "executable": str(binary),
            }) + "\n"
        self.fail(f"Unexpected command in compiler-free unit test: {arguments!r}")

    def fake_execute(self, command, cwd, env=None):
        del cwd, env
        arguments = [str(argument) for argument in command]
        self.assertEqual(arguments[1], "--external")
        self.assertEqual(arguments[2], "--output")
        self.assertEqual(len(arguments), 4)
        directory = pathlib.Path(arguments[3])
        directory.mkdir(parents=True, exist_ok=True)
        for name in ["development.json", "development.md", "development-predictions.jsonl"]:
            if self.missing_artifact and name.endswith(".jsonl"):
                continue
            (directory / name).write_text("synthetic guard test output\n", encoding="utf8")
        if self.extra_artifact:
            (directory / "unrequested-holdout.json").write_text("must be rejected", encoding="utf8")
        if self.execution_mutation:
            self.execution_mutation()
        if self.fail_execution:
            raise subprocess.CalledProcessError(1, arguments, output="failed after a partial write")
        return "external development test execution\n"

    def build(self):
        return verified.build(self.build_dir, "1.85.0", root=self.root)

    def run_evaluation(self):
        return verified.run(self.build_dir, self.output, root=self.root)

    def new_output(self):
        self.output_number += 1
        self.output = self.root / "target" / f"verified-results-{self.output_number}"

    def assert_no_success(self):
        self.assertFalse((self.output / verified.RUN_RECORD).exists())

    def append(self, relative):
        with (self.root / relative).open("ab") as stream:
            stream.write(b"\nchanged after the recorded build\n")

    def test_input_inventory_covers_scoped_sources_and_embedded_external_inputs(self):
        inventory = verified.input_inventory(self.root)
        for path in [
            "src/engine.rs", "evaluation/src/main.rs", "evaluation/verified.py",
            "Cargo.toml", "Cargo.lock", "README.md", "evaluation/Cargo.toml",
            "evaluation/external-protocol.json", "evaluation/external/t2d-v1/corpus.json",
            "evaluation/external/t2d-v1/provenance.json", "evaluation/import_t2d.py",
        ]:
            self.assertIn(path, inventory)
            self.assertEqual(inventory[path], verified.raw_sha256(self.root / path))
        self.assertEqual(list(inventory), sorted(inventory))
        self.assertFalse(any(path.startswith("target/") for path in inventory))

    def test_inventory_detects_nested_additions_deletions_and_optional_build_script(self):
        before = verified.input_inventory(self.root)
        nested = self.root / "src" / "nested" / "new.rs"
        nested.parent.mkdir()
        nested.write_text("pub fn new_module() {}\n", encoding="utf8")
        self.assertIn("src/nested/new.rs", verified.input_inventory(self.root))
        nested.unlink()
        self.assertEqual(before, verified.input_inventory(self.root))
        (self.root / "build.rs").write_text("fn main() {}\n", encoding="utf8")
        self.assertNotEqual(before, verified.input_inventory(self.root))

    def test_raw_hash_distinguishes_line_endings_and_binary_content(self):
        artifact = self.root / "target-byte-test"
        artifact.write_bytes(b"one\r\ntwo\x00")
        self.assertEqual(verified.raw_sha256(artifact), hashlib.sha256(artifact.read_bytes()).hexdigest())
        original = verified.raw_sha256(artifact)
        artifact.write_bytes(b"one\ntwo\x00")
        self.assertNotEqual(original, verified.raw_sha256(artifact))

    def test_build_and_run_execute_only_the_fixed_external_development_command(self):
        self.build()
        self.run_evaluation()
        self.execute_mock.assert_called_once()
        self.assertEqual(
            {path.name for path in self.output.iterdir()}, {"artifacts", verified.RUN_RECORD}
        )
        self.assertEqual(
            {path.name for path in (self.output / "artifacts").iterdir()}, set(verified.ARTIFACTS)
        )
        record = json.loads((self.output / verified.RUN_RECORD).read_text(encoding="utf8"))
        self.assertEqual(record["status"], "success")
        self.assertEqual(record["partition"], "development")
        self.assertEqual(set(record["artifact_sha256"]), set(verified.ARTIFACTS))
        for name, digest in record["artifact_sha256"].items():
            self.assertEqual(digest, verified.raw_sha256(self.output / "artifacts" / name))

    def test_source_change_after_build_rejects_before_execution(self):
        self.build()
        self.append("src/engine.rs")
        with self.assertRaises(ValueError):
            self.run_evaluation()
        self.execute_mock.assert_not_called()
        self.assert_no_success()

    def test_changed_protocol_fixture_manifest_lock_and_runner_reject_before_execution(self):
        self.build()
        for relative in [
            "evaluation/external-protocol.json", "evaluation/external/t2d-v1/corpus.json",
            "evaluation/external/t2d-v1/provenance.json", "Cargo.toml", "Cargo.lock", "README.md",
            "evaluation/Cargo.toml", "evaluation/verified.py", "evaluation/import_t2d.py",
        ]:
            with self.subTest(relative=relative):
                self.new_output()
                path = self.root / relative
                original = path.read_bytes()
                try:
                    self.append(relative)
                    with self.assertRaises(ValueError):
                        self.run_evaluation()
                    self.execute_mock.assert_not_called()
                    self.assert_no_success()
                finally:
                    path.write_bytes(original)

    def test_added_nested_source_and_deleted_existing_source_reject_before_execution(self):
        self.build()
        nested = self.root / "evaluation" / "src" / "new_module" / "new.rs"
        nested.parent.mkdir()
        nested.write_text("fn added() {}", encoding="utf8")
        with self.assertRaises(ValueError):
            self.run_evaluation()
        nested.unlink()
        self.new_output()
        existing = self.root / "src" / "engine.rs"
        original = existing.read_bytes()
        try:
            existing.unlink()
            with self.assertRaises(ValueError):
                self.run_evaluation()
        finally:
            existing.write_bytes(original)
        self.execute_mock.assert_not_called()

    def test_modified_recorded_binary_rejects_before_execution(self):
        self.build()
        binary = self.build_dir / "bin" / ("fieldkin-eval.exe" if os.name == "nt" else "fieldkin-eval")
        self.assertTrue(binary.is_file())
        binary.write_bytes(b"different executable")
        with self.assertRaises(ValueError):
            self.run_evaluation()
        self.execute_mock.assert_not_called()

    def test_changed_cargo_config_or_build_environment_rejects_before_execution(self):
        self.build()
        config = self.root / ".cargo" / "config.toml"
        config.parent.mkdir()
        config.write_text("# added Cargo configuration\n", encoding="utf8")
        with self.assertRaises(ValueError):
            self.run_evaluation()
        config.unlink()
        self.new_output()
        with mock.patch.dict(os.environ, {"RUSTFLAGS": "--cfg fieldkin_changed_test"}):
            with self.assertRaises(ValueError):
                self.run_evaluation()
        self.execute_mock.assert_not_called()

    def test_build_environment_hashes_values_without_collecting_credentials(self):
        first = verified.build_environment({
            "RUSTFLAGS": "--cfg private_configuration_value",
            "CARGO_REGISTRIES_CRATES_IO_TOKEN": "a-private-credential",
            "UNRELATED_SETTING": "irrelevant",
        })
        second = verified.build_environment({"RUSTFLAGS": "--cfg different_value"})
        self.assertNotEqual(first, second)
        encoded = json.dumps(first)
        self.assertNotIn("private_configuration_value", encoded)
        self.assertNotIn("a-private-credential", encoded)
        self.assertNotIn("CARGO_REGISTRIES_CRATES_IO_TOKEN", first)
        self.assertNotIn("UNRELATED_SETTING", first)

    def test_compiler_target_and_unscoped_config_overrides_are_rejected(self):
        for key in ["RUSTC", "RUSTC_WRAPPER", "CARGO_BUILD_RUSTC", "CARGO_BUILD_TARGET"]:
            with self.subTest(environment=key):
                with self.assertRaises(ValueError):
                    verified.build_environment({key: "untracked-override"})
        config = self.root / ".cargo" / "config.toml"
        config.parent.mkdir()
        for value in [
            'include = "untracked.toml"\n',
            '[source.crates-io]\nreplace-with = "untracked"\n',
            '[build]\ntarget = "wasm32-unknown-unknown"\n',
            '[build]\nrustc = "untracked-compiler"\n',
            '[env]\nRUSTC_WRAPPER = "untracked-wrapper"\n',
        ]:
            with self.subTest(config=value):
                config.write_text(value, encoding="utf8")
                with self.assertRaises(ValueError):
                    verified.cargo_config_inventory(self.root)

    def test_manifest_targets_must_remain_inside_the_inventoried_source_trees(self):
        for relative, declaration, source_directory in [
            ("Cargo.toml", "[lib]", "src"),
            ("evaluation/Cargo.toml", '[[bin]]\nname = "fieldkin-eval"', "evaluation/src"),
        ]:
            with self.subTest(manifest=relative):
                manifest = self.root / relative
                original = manifest.read_text(encoding="utf8")
                try:
                    manifest.write_text(original + f'\n{declaration}\npath = "alternate.rs"\n', encoding="utf8")
                    with self.assertRaises(ValueError):
                        verified.input_inventory(self.root)
                    alternate = self.root / source_directory / "alternate.rs"
                    alternate.write_text("fn main() {}\n", encoding="utf8")
                    manifest.write_text(original + f'\n{declaration}\npath = "src/alternate.rs"\n', encoding="utf8")
                    self.assertIn(f"{source_directory}/alternate.rs", verified.input_inventory(self.root))
                finally:
                    manifest.write_text(original, encoding="utf8")

    def test_ancestor_and_cargo_home_configs_are_recorded_and_detect_changes(self):
        ancestor = self.root.parent / ".cargo" / "config.toml"
        cargo_home = pathlib.Path(os.environ["CARGO_HOME"]) / "config.toml"
        for config in [ancestor, cargo_home]:
            config.parent.mkdir(parents=True, exist_ok=True)
            config.write_text("# local compiler configuration\n", encoding="utf8")
        inventory = verified.cargo_config_inventory(self.root)
        for path in [ancestor, cargo_home]:
            self.assertEqual(inventory[str(path)], verified.raw_sha256(path))
        self.build()
        for path in [ancestor, cargo_home]:
            with self.subTest(path=path):
                self.new_output()
                original = path.read_bytes()
                try:
                    path.write_bytes(original + b"# changed configuration\n")
                    with self.assertRaises(ValueError):
                        self.run_evaluation()
                    self.execute_mock.assert_not_called()
                    self.assert_no_success()
                finally:
                    path.write_bytes(original)

    def test_symlink_or_windows_reparse_component_is_rejected_before_hashing(self):
        blocked = self.root / "src" / "engine.rs"
        actual_lstat = pathlib.Path.lstat

        def reparse(path):
            metadata = actual_lstat(path)
            if path == blocked:
                return types.SimpleNamespace(st_mode=metadata.st_mode, st_file_attributes=0x400)
            return metadata

        with mock.patch.object(pathlib.Path, "lstat", reparse):
            with mock.patch.object(verified.stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0x400, create=True):
                with self.assertRaises(ValueError):
                    verified.raw_sha256(blocked)
        with mock.patch.object(pathlib.Path, "is_symlink", return_value=True):
            with self.assertRaises(ValueError):
                verified.input_inventory(self.root)

    def test_tampered_build_record_rejects_before_execution(self):
        self.build()
        path = self.build_dir / "build.json"
        record = json.loads(path.read_text(encoding="utf8"))
        record["protocol"] = "unrecognized-protocol"
        path.write_text(json.dumps(record), encoding="utf8")
        with self.assertRaises(ValueError):
            self.run_evaluation()
        self.execute_mock.assert_not_called()

    def test_build_drift_cannot_produce_a_successful_build_record(self):
        self.compilation_mutation = lambda: self.append("src/engine.rs")
        with self.assertRaises(ValueError):
            self.build()
        self.assertFalse((self.build_dir / "build.json").exists())

    def test_run_drift_cannot_publish_successful_evidence(self):
        self.build()
        self.execution_mutation = lambda: self.append("evaluation/external-protocol.json")
        with self.assertRaises(ValueError):
            self.run_evaluation()
        self.assert_no_success()

    def test_partial_process_failure_cannot_publish_successful_evidence(self):
        self.build()
        self.fail_execution = True
        with self.assertRaises((ValueError, subprocess.CalledProcessError)):
            self.run_evaluation()
        self.assert_no_success()
        self.assertTrue((self.output / "artifacts" / "development.json").is_file())
        previous_calls = self.execute_mock.call_count
        self.fail_execution = False
        with self.assertRaises(ValueError):
            self.run_evaluation()
        self.assertEqual(previous_calls, self.execute_mock.call_count)
        self.assert_no_success()

    def test_binary_drift_during_execution_cannot_publish_successful_evidence(self):
        self.build()
        binary = self.build_dir / "bin" / ("fieldkin-eval.exe" if os.name == "nt" else "fieldkin-eval")
        self.execution_mutation = lambda: binary.write_bytes(b"replaced during the process")
        with self.assertRaises(ValueError):
            self.run_evaluation()
        self.assert_no_success()

    def test_missing_or_extra_artifact_is_rejected(self):
        self.build()
        for missing, extra in [(True, False), (False, True)]:
            with self.subTest(missing=missing, extra=extra):
                self.new_output()
                self.missing_artifact = missing
                self.extra_artifact = extra
                with self.assertRaises(ValueError):
                    self.run_evaluation()
                self.assert_no_success()

    def test_oversized_artifact_is_rejected_without_a_success_record(self):
        self.build()
        with mock.patch.object(verified, "MAX_ARTIFACT_BYTES", 4):
            with self.assertRaises(ValueError):
                self.run_evaluation()
        self.assert_no_success()

    def test_existing_output_and_build_directories_are_never_overwritten(self):
        self.build()
        original = (self.build_dir / "build.json").read_bytes()
        with self.assertRaises((ValueError, FileExistsError)):
            self.build()
        self.assertEqual(original, (self.build_dir / "build.json").read_bytes())
        self.output.mkdir()
        sentinel = self.output / "owner-file"
        sentinel.write_bytes(b"keep exactly")
        with self.assertRaises((ValueError, FileExistsError)):
            self.run_evaluation()
        self.execute_mock.assert_not_called()
        self.assertEqual(b"keep exactly", sentinel.read_bytes())

    def test_outside_and_overlapping_destinations_reject_before_execution(self):
        self.build()
        for output in [
            self.root.parent / "outside-output", self.root / "target",
            self.build_dir, self.build_dir / "nested-output",
            self.root / "target" / ".." / "outside-output",
        ]:
            with self.subTest(output=output):
                with self.assertRaises(ValueError):
                    verified.run(self.build_dir, output, root=self.root)
        for build_dir in [self.root / "evaluation" / "results" / "invalid-build", self.root.parent / "outside-build"]:
            with self.subTest(build_dir=build_dir):
                with self.assertRaises(ValueError):
                    verified.build(build_dir, "1.85.0", root=self.root)
        self.execute_mock.assert_not_called()

    def test_cli_rejects_holdout_and_arbitrary_evaluator_arguments(self):
        for flag in ["--acknowledge-holdout", "--split", "--external", "--arbitrary-evaluator-option"]:
            with self.subTest(flag=flag):
                with mock.patch.object(sys, "argv", ["verified.py", "run", "--build-dir", str(self.build_dir), "--output", str(self.output), flag]):
                    with mock.patch("sys.stderr", new_callable=io.StringIO):
                        with self.assertRaises(SystemExit) as failure:
                            verified.main()
                    self.assertEqual(failure.exception.code, 2)
                self.capture_mock.assert_not_called()
                self.execute_mock.assert_not_called()


if __name__ == "__main__":
    unittest.main()
