"""Dependency-free negative checks for fail-closed release provenance."""

import copy
import io
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest

import release_evidence as release


class ReleaseEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="fieldkin-release-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "Cargo.toml").write_text('[package]\nname="fieldkin"\nversion="0.2.1"\n')
        (self.root / "Cargo.lock").write_text('[[package]]\nname="ort"\nversion="2.0.0-rc.13"\n')
        for name in ("release-requirements.txt", "release-model.sha256", "release-nltk.sha256"):
            path = self.root / "validation" / name
            path.parent.mkdir(exist_ok=True)
            path.write_text("pinned inputs\n")
        for name in release.EXPECTED_REFERENCE_PATCHES:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("source patch\n")
        self.git("init", "-q")
        self.git("add", ".")
        self.git("-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "-qm", "fixture")
        self.sha = self.git("rev-parse", "HEAD")
        self.inputs = self.root / "artifacts"
        self.jobs = {name: {"result": "success"}
                     for name in ("preflight", "ordinary-ci", "release-validation")}
        package = self.artifact("validated-package")
        archive = package / "fieldkin-0.2.1.crate"
        with tarfile.open(archive, "w:gz") as stream:
            contents = {"Cargo.toml": (self.root / "Cargo.toml").read_bytes(),
                        ".cargo_vcs_info.json": json.dumps({"git": {"sha1": self.sha}}).encode()}
            for name, data in contents.items():
                member = tarfile.TarInfo("fieldkin-0.2.1/" + name)
                member.size = len(data)
                stream.addfile(member, io.BytesIO(data))
        self.package = {**release.archive_record(self.root, archive, self.sha),
                        "rustc": "rustc 1.99.0 (fixture)\nrelease: 1.99.0", "cargo": "cargo 1.99.0 (fixture)"}
        self.write(package / "package.json", self.package)
        reference = self.artifact("live-reference")
        provenance = {"verified": True, "mode": "fixed", "patches": [
            {"path": name, "verified": True, "recorded_sha256": release.sha256(self.root / name),
             "actual_sha256": release.sha256(self.root / name)} for name in release.EXPECTED_REFERENCE_PATCHES]}
        for name in ("base_source", "source"):
            provenance[f"recorded_{name}_sha256"] = "a" * 64
            provenance[f"actual_{name}_sha256"] = "a" * 64
        self.fixed = {"aggregate": {"case_count": 6, "all_equivalent": True},
                      "metadata": {"reference_provenance": provenance, "context_verified": True,
                                   "expected_upstream_revision": release.EXPECTED_UPSTREAM_REVISION},
                      "coverage": {"complete": True, "expected_case_count": 6}}
        self.write(reference / "comparison-fixed.json", self.fixed)
        self.write(reference / "comparison-upstream.json", {
            "aggregate": {"case_count": 6}, "upstream_diagnostic": {"passed": True}})
        self.write(reference / "python-fixed.json", {
            "upstream_revision": release.EXPECTED_UPSTREAM_REVISION, "python_version": "3.12.14",
            "dependency_versions": {"nltk": "3.9.2"}, "runtime_dependencies": {"pulp": {"version": "3.3.0"}}})
        (reference / "setup.log").write_text("PuLP 3.3.0 CBC SHA256 " + "a" * 64 + "\nVersion: 2.10.3\n")
        (reference / "distribution-patch.log").write_text("Ran 3 tests in 0.1s\n\nOK\n")
        self.write(reference / "cupid-regression.json", {"runs": [{"scores": [0.5]}] * 3})
        native = self.artifact("native-minilm")
        (native / "native-minilm.log").write_text("test result: ok. 1 passed; 0 failed; 0 ignored;\n")
        self.write(native / "model.json", {"source_commit": self.sha, "revision": "b" * 40})
        for compiler in ("1.94.0", "stable"):
            (self.artifact(f"packaged-consumer-{compiler}") / "packaged-consumer.log").write_text(
                self.sha + "\nPackaged default execution and optional-feature compilation passed\n")

    def git(self, *arguments):
        return subprocess.check_output(["git", *arguments], cwd=self.root, text=True,
                                       stderr=subprocess.DEVNULL).strip()

    def artifact(self, name):
        path = self.inputs / f"{name}-{self.sha}"
        path.mkdir(parents=True, exist_ok=True)
        return path

    @staticmethod
    def write(path, data):
        path.write_text(json.dumps(data))

    def collect(self, jobs=None):
        return release.collect(self.root, self.inputs, self.sha, self.jobs if jobs is None else jobs)

    def test_complete_dry_run_has_no_publication_tag(self):
        result = self.collect()
        self.assertEqual(result["source_commit"], self.sha)
        self.assertIsNone(result["tag"])
        self.assertFalse(result["publication_requested"])
        self.assertEqual(result["executed"]["native_minilm_tests"], 1)

    def test_publication_requires_existing_version_tag_at_exact_source(self):
        with self.assertRaisesRegex(ValueError, "requires an existing"):
            release.preflight(self.root, self.sha, publish=True)
        with self.assertRaisesRegex(ValueError, "does not match package version"):
            release.preflight(self.root, self.sha, "v99.0.0", True)
        self.git("tag", "v0.2.1")
        self.assertEqual(release.preflight(self.root, self.sha, "v0.2.1", True)["tag_commit"], self.sha)
        self.git("-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "--allow-empty", "-qm", "next")
        current = self.git("rev-parse", "HEAD")
        with self.assertRaisesRegex(ValueError, "not " + current):
            release.preflight(self.root, current, "v0.2.1", True)

    def test_wrong_revision_and_dirty_source_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "differs from intended source"):
            release.preflight(self.root, "f" * 40)
        with self.assertRaisesRegex(ValueError, "full commit SHA"):
            release.preflight(self.root, self.sha[:8])
        with (self.root / "Cargo.toml").open("a") as stream:
            stream.write("# changed source\n")
        with self.assertRaises(subprocess.CalledProcessError):
            release.preflight(self.root, self.sha)

    def test_every_prerequisite_must_explicitly_succeed(self):
        for name in self.jobs:
            for result in (None, "failure", "cancelled", "skipped", ""):
                jobs = copy.deepcopy(self.jobs)
                jobs[name] = {} if result is None else {"result": result}
                with self.subTest(name=name, result=result), self.assertRaisesRegex(ValueError, "did not succeed"):
                    self.collect(jobs)
        with self.assertRaisesRegex(ValueError, "did not succeed"):
            self.collect({})

    def test_missing_artifact_or_required_report_fails(self):
        shutil.rmtree(self.artifact("native-minilm"))
        with self.assertRaisesRegex(ValueError, "missing artifact"):
            self.collect()
        self.artifact("native-minilm")
        with self.assertRaises(FileNotFoundError):
            self.collect()

    def test_incomplete_or_corrupt_corrected_reference_is_rejected(self):
        path = self.artifact("live-reference") / "comparison-fixed.json"
        corruptions = [lambda report: report["aggregate"].update(case_count=0),
                       lambda report: report["aggregate"].update(all_equivalent=False),
                       lambda report: report["coverage"].update(complete=False),
                       lambda report: report["metadata"]["reference_provenance"].update(verified=False),
                       lambda report: report["metadata"]["reference_provenance"].update(actual_source_sha256="b" * 64),
                       lambda report: report["metadata"]["reference_provenance"]["patches"][0].update(recorded_sha256="c" * 64)]
        for corrupt in corruptions:
            report = copy.deepcopy(self.fixed)
            corrupt(report)
            self.write(path, report)
            with self.subTest(corruption=corrupt), self.assertRaises(ValueError):
                self.collect()

    def test_zero_or_ignored_inference_is_not_success(self):
        path = self.artifact("native-minilm") / "native-minilm.log"
        for log in ("test result: ok. 0 passed; 0 failed; 0 ignored;",
                    "test result: ok. 0 passed; 0 failed; 1 ignored;"):
            path.write_text(log)
            with self.assertRaisesRegex(ValueError, "exactly once"):
                self.collect()

    def test_missing_patch_tests_or_upstream_failure_is_not_success(self):
        path = self.artifact("live-reference") / "distribution-patch.log"
        path.write_text("Ran 0 tests in 0.0s\n\nOK\n")
        with self.assertRaisesRegex(ValueError, "did not execute"):
            self.collect()
        path.write_text("Ran 3 tests in 0.1s\n\nOK\n")
        self.write(self.artifact("live-reference") / "comparison-upstream.json", {
            "aggregate": {"case_count": 6}, "upstream_diagnostic": {"passed": False}})
        with self.assertRaisesRegex(ValueError, "upstream diagnostic"):
            self.collect()

    def test_package_revision_version_checksum_inventory_and_toolchain_must_match(self):
        release.check_package(self.package, copy.deepcopy(self.package))
        for key in ("source_commit", "version", "sha256", "files", "rustc", "cargo"):
            wrong = copy.deepcopy(self.package)
            wrong[key] = "changed"
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, key):
                release.check_package(self.package, wrong)
        path = self.artifact("validated-package") / "package.json"
        for key in ("source_commit", "version", "sha256", "files"):
            wrong = copy.deepcopy(self.package)
            wrong[key] = "changed"
            self.write(path, wrong)
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.collect()


if __name__ == "__main__":
    unittest.main()
