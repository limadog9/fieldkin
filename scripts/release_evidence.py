#!/usr/bin/env python3
"""Small release preflight and same-run evidence checks; never uploads a crate."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tarfile
import tomllib

from compare_valentine_parity import EXPECTED_REFERENCE_PATCHES, EXPECTED_UPSTREAM_REVISION


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def preflight(root, revision, tag="", publish=False):
    require(re.fullmatch(r"[0-9a-f]{40}", revision), "source revision must be a full commit SHA")
    actual = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    require(actual == revision, f"checkout {actual} differs from intended source {revision}")
    subprocess.run(["git", "diff", "--exit-code", "HEAD", "--"], cwd=root, check=True,
                   stdout=subprocess.DEVNULL)
    package = tomllib.loads((root / "Cargo.toml").read_text())["package"]
    require(package["name"] == "fieldkin", "unexpected package name")
    version = package["version"]
    require(re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?", version), "invalid version")
    require(not publish or tag, "publication requires an existing release tag")
    tag_commit = None
    if tag:
        require(tag == f"v{version}", f"tag {tag} does not match package version {version}")
        tag_commit = subprocess.check_output(
            ["git", "rev-parse", "--verify", f"refs/tags/{tag}^{{commit}}"], cwd=root, text=True).strip()
        require(tag_commit == revision, f"tag {tag} points at {tag_commit}, not {revision}")
    return {"source_commit": revision, "version": version, "tag": tag or None,
            "tag_commit": tag_commit, "publication_requested": publish}


def archive_record(root, archive, revision):
    source = preflight(root, revision)
    with tarfile.open(archive, "r:gz") as stream:
        members = [member for member in stream.getmembers() if member.isfile()]
        require(len({member.name for member in members}) == len(members), "duplicate archive entries")
        prefix = f"fieldkin-{source['version']}/"
        require(members and all(member.name.startswith(prefix) and ".." not in Path(member.name).parts
                                for member in members), "unexpected archive paths")
        data = {member.name.removeprefix(prefix): stream.extractfile(member).read() for member in members}
    metadata = tomllib.loads(data["Cargo.toml"].decode())["package"]
    require(metadata["name"] == "fieldkin" and metadata["version"] == source["version"],
            "archive package name/version differs from source")
    vcs = json.loads(data[".cargo_vcs_info.json"])["git"]
    require(vcs["sha1"] == revision and not vcs.get("dirty", False), "archive source revision is not clean/current")
    return {**source, "archive": archive.name, "sha256": sha256(archive),
            "files": {name: hashlib.sha256(value).hexdigest() for name, value in sorted(data.items())}}


def package_record(root, archive, revision):
    return {**archive_record(root, archive, revision),
            "rustc": subprocess.check_output(["rustc", "-Vv"], text=True).strip(),
            "cargo": subprocess.check_output(["cargo", "-V"], text=True).strip()}


def check_package(expected, actual):
    for key in ("source_commit", "version", "sha256", "files", "rustc", "cargo"):
        require(expected.get(key) and expected[key] == actual.get(key), f"package {key} differs from validated artifact")


def collect(root, inputs, revision, jobs, tag="", publish=False):
    source = preflight(root, revision, tag, publish)
    for name in ("preflight", "ordinary-ci", "release-validation"):
        require(jobs.get(name, {}).get("result") == "success", f"required job {name} did not succeed")

    def artifact(name):
        path = inputs / f"{name}-{revision}"
        require(path.is_dir(), f"missing artifact {name} for source {revision}")
        return path

    reference = artifact("live-reference")
    fixed = json.loads((reference / "comparison-fixed.json").read_text())
    require(fixed["aggregate"]["case_count"] > 0 and fixed["aggregate"]["all_equivalent"] is True,
            "corrected-reference comparison is empty or failed")
    provenance = fixed["metadata"]["reference_provenance"]
    require(provenance["verified"] is True and provenance["mode"] == "fixed"
            and fixed["metadata"]["context_verified"] is True
            and fixed["metadata"]["expected_upstream_revision"] == EXPECTED_UPSTREAM_REVISION
            and fixed["coverage"]["complete"] is True
            and fixed["aggregate"]["case_count"] == fixed["coverage"]["expected_case_count"],
            "missing verified corrected-reference provenance/coverage")
    for name in ("base_source", "source"):
        digest = provenance[f"recorded_{name}_sha256"]
        require(re.fullmatch(r"[0-9a-f]{64}", digest)
                and digest == provenance[f"actual_{name}_sha256"], "corrupt reference source checksum")
    patches = provenance["patches"]
    require(sorted(patch["path"] for patch in patches) == sorted(EXPECTED_REFERENCE_PATCHES),
            "unexpected reference patches")
    for patch in patches:
        require(patch["verified"] is True and patch["recorded_sha256"] == patch["actual_sha256"]
                == sha256(root / patch["path"]), "corrupt reference patch checksum")
    upstream = json.loads((reference / "comparison-upstream.json").read_text())
    require(upstream["upstream_diagnostic"]["passed"] is True and upstream["aggregate"]["case_count"] > 0,
            "unmodified-upstream diagnostic is empty or failed")
    distribution = (reference / "distribution-patch.log").read_text()
    require(re.search(r"Ran [1-9][0-9]* tests? in", distribution) and re.search(r"\nOK\s*$", distribution),
            "distribution reference-patch regressions did not execute successfully")
    cupid = json.loads((reference / "cupid-regression.json").read_text())
    require(len(cupid["runs"]) == 3 and all(run.get("scores") for run in cupid["runs"]),
            "missing Cupid source-patch regressions")
    native_artifact = artifact("native-minilm")
    native = (native_artifact / "native-minilm.log").read_text()
    model = json.loads((native_artifact / "model.json").read_text())
    require(model["source_commit"] == revision and re.fullmatch(r"[0-9a-f]{40}", model["revision"]),
            "invalid model provenance")
    require("test result: ok. 1 passed; 0 failed; 0 ignored;" in native,
            "real native MiniLM test did not execute exactly once")
    consumers = {}
    for compiler in ("1.94.0", "stable"):
        log = (artifact(f"packaged-consumer-{compiler}") / "packaged-consumer.log").read_text()
        require(f"{revision}\n" in log and
                "Packaged default execution and optional-feature compilation passed" in log,
                f"missing successful packaged consumption on {compiler}")
        rustc = re.findall(r"^rustc .+$", log, re.MULTILINE)
        cargo = re.findall(r"^cargo .+$", log, re.MULTILINE)
        checksum = re.findall(r"^SHA256: ([0-9a-f]{64})$", log, re.MULTILINE)
        dependencies = {name: {"version": version, "rust_version": minimum}
                        for name, version, minimum in re.findall(
                            r"^  (fastembed|ort|ort-sys|polars-core|good_lp|microlp) ([^ ]+) \(rust-version=([^)]*)\)$",
                            log, re.MULTILINE)}
        require(len(rustc) == len(cargo) == len(checksum) == 1
                and len(dependencies) == 6, f"incomplete packaged consumer provenance on {compiler}")
        consumers[compiler] = {"rustc": rustc[0], "cargo": cargo[0],
                               "archive_sha256": checksum[0], "dependencies": dependencies}
    package_path = artifact("validated-package")
    package = json.loads((package_path / "package.json").read_text())
    archive = package_path / package["archive"]
    require(package["source_commit"] == revision and package["version"] == source["version"],
            "validated package revision/version differs from release")
    require(package["sha256"] == sha256(archive), "validated package checksum mismatch")
    require(consumers["stable"]["archive_sha256"] == package["sha256"],
            "stable consumer did not consume the validated package")
    actual = archive_record(root, archive, revision)
    for key in ("source_commit", "version", "sha256", "files"):
        require(package[key] == actual[key], f"validated package {key} mismatch")
    python = json.loads((reference / "python-fixed.json").read_text())
    require(python["upstream_revision"] == EXPECTED_UPSTREAM_REVISION and python["dependency_versions"],
            "missing Python dependency/reference provenance")
    dependencies = tomllib.loads((root / "Cargo.lock").read_text())["package"]
    solver = [line for line in (reference / "setup.log").read_text().splitlines()
              if "CBC SHA256" in line or line.startswith("Version: ")]
    require(any("CBC SHA256" in line for line in solver), "missing CBC binary provenance")
    pins = ["Cargo.lock", "validation/release-requirements.txt", "validation/release-model.sha256",
            "validation/release-nltk.sha256"]
    return {**source, "workflow_run": None, "required_jobs": {key: jobs[key]["result"] for key in jobs},
            "package": package, "packaged_consumers": consumers, "corrected_reference": fixed["aggregate"],
            "reference_provenance": provenance, "unmodified_upstream": upstream["upstream_diagnostic"],
            "executed": {"distribution_patch_regressions": True, "cupid_lookup_orders": len(cupid["runs"]),
                         "native_minilm_tests": 1, "packaged_consumers": ["1.94.0", "stable"],
                         "optional_consumer_features": "compilation only"},
            "input_checksums": {name: sha256(root / name) for name in pins},
            "model": {**model, "files": (root / "validation/release-model.sha256").read_text()},
            "python": {"version": python["python_version"], "dependencies": python["dependency_versions"],
                       "runtime_modules": python["runtime_dependencies"], "solver": solver},
            "native_dependencies": {item["name"]: item["version"] for item in dependencies
                                    if item["name"] in ("fastembed", "ort", "ort-sys", "good_lp", "microlp")},
            "boundary": "Corrected-reference corpus agreement is not universal upstream output parity; Distribution equal-optimum pairs may differ."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("preflight", "package", "collect", "check-package"))
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--sha", required=True)
    parser.add_argument("--tag", default="")
    parser.add_argument("--publish", choices=("true", "false"), default="false")
    parser.add_argument("--archive", type=Path)
    parser.add_argument("--expected", type=Path)
    parser.add_argument("--inputs", type=Path)
    parser.add_argument("--jobs", default="{}")
    parser.add_argument("--run-url")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.operation == "preflight":
        result = preflight(args.root, args.sha, args.tag, args.publish == "true")
    elif args.operation in ("package", "check-package"):
        result = package_record(args.root, args.archive, args.sha)
        if args.operation == "check-package":
            check_package(json.loads(args.expected.read_text()), result)
    else:
        result = collect(args.root, args.inputs, args.sha, json.loads(args.jobs), args.tag, args.publish == "true")
        result["workflow_run"] = args.run_url
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(f"Verified {args.operation}: {result.get('source_commit')}; wrote {args.output}")


if __name__ == "__main__":
    main()
