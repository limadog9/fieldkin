#!/usr/bin/env python3
"""Opt-in, development-only external evaluation with a verified build record.

Requires Python 3.11+. Records raw file hashes, not normalized text hashes.
This detects accidental stale binaries and changing local inputs. It is not a
signature, a hermetic build, or protection against a hostile local process that
can replace this script, the toolchain, dependencies, and records together.
"""

import argparse
import datetime
import hashlib
import json
import os
import pathlib
import re
import shutil
import stat
import subprocess
import sys

if sys.version_info < (3, 11):
    raise SystemExit("Verified evaluation requires Python 3.11 or newer (on Windows: py -3.11).")

import tomllib


ROOT = pathlib.Path(__file__).resolve().parent.parent
PROTOCOL = "fieldkin-external-verified-v1"
ARTIFACTS = ("development.json", "development-predictions.jsonl", "development.md")
BUILD_RECORD = "build.json"
RUN_RECORD = "run.json"
MAX_ARTIFACT_BYTES = 64 * 1024 * 1024
MAX_RECORD_BYTES = 4 * 1024 * 1024
TRUST_BOUNDARY = (
    "Local reproducibility record, not a signature or hermetic build. Trust the "
    "wrapper, local processes, compiler, Cargo and locked dependency cache. "
    "Before/after checks cannot detect a change restored between checks. "
    "Only the scoped source/configuration inputs are inventoried; arbitrary "
    "build-script or compiler accesses outside that scope are not attested."
)


class VerificationError(ValueError):
    """An input, executable, directory or record failed verification."""


def _root(root):
    return pathlib.Path(ROOT if root is None else root).absolute()


def _checked_path(path):
    """Check components before resolving: resolving first hides symlinks."""
    path = pathlib.Path(os.path.abspath(path))
    for component in (*reversed(path.parents), path):
        try:
            attributes = getattr(component.lstat(), "st_file_attributes", 0)
        except FileNotFoundError:
            attributes = 0
        if component.is_symlink() or attributes & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0) or (
            hasattr(component, "is_junction") and component.is_junction()
        ):
            raise VerificationError("symlinks and junctions are not supported")
    return path


def _regular_file(path):
    path = _checked_path(path)
    if not stat.S_ISREG(path.stat().st_mode):
        raise VerificationError("expected a regular input file")
    return path


def raw_sha256(path):
    """Hash file bytes without line-ending or encoding normalization."""
    path = _regular_file(path)
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def _files(directory):
    directory = _checked_path(directory)
    if not directory.is_dir():
        raise VerificationError("required input directory is missing")
    result = []
    for entry in sorted(directory.iterdir()):
        _checked_path(entry)
        if entry.is_dir():
            result.extend(_files(entry))
        else:
            result.append(_regular_file(entry))
    return result


def _toml(path):
    with _regular_file(path).open("rb") as stream:
        return tomllib.load(stream)


def _validate_manifests(root):
    for relative in ("Cargo.toml", "evaluation/Cargo.toml"):
        path = root / relative
        manifest = _toml(path)
        if any(key in manifest for key in ("patch", "replace")):
            raise VerificationError("manifest dependency overrides are unsupported")
        build_script = manifest.get("package", {}).get("build")
        if build_script not in (None, False, "build.rs"):
            raise VerificationError("only the standard build.rs path is inventoried")
        targets = [manifest.get("lib", {}), *manifest.get("bin", [])]
        for target in targets:
            if "path" in target:
                target_path = _checked_path(path.parent / target["path"])
                if not _within(target_path, path.parent / "src"):
                    raise VerificationError("Cargo source targets must remain within the inventoried src directory")
        if relative == "Cargo.toml":
            workspace = manifest.get("workspace", {})
            if workspace.get("members") != ["evaluation"]:
                raise VerificationError("verified builds require the scoped evaluation workspace")

        def visit(value):
            if not isinstance(value, dict):
                return
            for key, child in value.items():
                if key in ("dependencies", "dev-dependencies", "build-dependencies"):
                    for name, dependency in child.items():
                        if isinstance(dependency, dict) and "path" in dependency:
                            allowed = (
                                relative == "evaluation/Cargo.toml"
                                and name == "fieldkin"
                                and dependency["path"] == ".."
                            )
                            if not allowed:
                                raise VerificationError("unscoped path dependencies are unsupported")
                visit(child)

        visit(manifest)


def input_inventory(root=None):
    """Inventory every file in the declared source/data scope, including additions."""
    root = _checked_path(_root(root))
    _validate_manifests(root)
    paths = []
    for relative in ("src", "evaluation/src", "evaluation/corpus", "evaluation/fixtures", "evaluation/external/t2d-v1", "evaluation/external/northix-v1"):
        paths.extend(_files(root / relative))
    for relative in ("Cargo.toml", "Cargo.lock", "README.md", "evaluation/Cargo.toml", "evaluation/import_t2d.py", "evaluation/verified.py", "evaluation/import_northix.py", "evaluation/record_northix.py", "evaluation/compare_valentine.py", "evaluation/valentine-requirements.txt", "evaluation/valentine-environment.json"):
        paths.append(_regular_file(root / relative))
    for directory in (root, root / "evaluation"):
        for name in ("Cargo.lock", "build.rs", "rust-toolchain", "rust-toolchain.toml"):
            path = _checked_path(directory / name)
            if path.exists():
                paths.append(_regular_file(path))
    protocols = sorted((root / "evaluation").glob("*protocol.json"))
    required = {"protocol.json", "stage3-protocol.json", "release-protocol.json", "corrective-protocol.json", "external-protocol.json", "northix-protocol.json"}
    if not required.issubset({path.name for path in protocols}):
        raise VerificationError("an evaluation protocol is missing")
    paths.extend(protocols)
    return dict(sorted((path.relative_to(root).as_posix(), raw_sha256(path)) for path in set(paths)))


def cargo_config_inventory(root=None, environ=None):
    """Record Cargo configs from cwd ancestors and CARGO_HOME, never credentials."""
    root = _checked_path(_root(root))
    environ = os.environ if environ is None else environ
    cargo_home = pathlib.Path(environ.get("CARGO_HOME", pathlib.Path.home() / ".cargo"))
    if not cargo_home.is_absolute():
        cargo_home = root / cargo_home
    directories = {directory / ".cargo" for directory in (root, *root.parents)}
    directories.add(cargo_home)
    result = {}
    for directory in sorted(directories):
        for name in ("config", "config.toml"):
            path = _checked_path(directory / name)
            if not path.exists():
                continue
            config = _toml(path)
            if any(key in config for key in ("include", "source", "paths", "patch", "replace")):
                raise VerificationError("Cargo includes and source/path overrides are unsupported")
            if config.get("build", {}).get("target") is not None:
                raise VerificationError("cross-target Cargo builds are unsupported")
            if any(key in config.get("build", {}) for key in ("rustc", "rustdoc", "rustc-wrapper", "rustc-workspace-wrapper")):
                raise VerificationError("Cargo compiler overrides are unsupported")
            if any(key in config.get("env", {}) for key in ("RUSTC", "RUSTDOC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_TARGET", "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTDOC", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER")):
                raise VerificationError("Cargo environment compiler/target overrides are unsupported")
            result[str(path)] = raw_sha256(path)
    # Explicit +toolchain selection is used, but track inherited toolchain files
    # as well so introducing one cannot silently change the recorded context.
    for directory in (root, *root.parents):
        for name in ("rust-toolchain", "rust-toolchain.toml"):
            path = _checked_path(directory / name)
            if path.exists():
                result[str(path)] = raw_sha256(path)
    return dict(sorted(result.items()))


def build_environment(environ=None):
    """Fingerprint build-affecting environment values; omit credential variables."""
    environ = os.environ if environ is None else environ
    if environ.get("CARGO_BUILD_TARGET"):
        raise VerificationError("CARGO_BUILD_TARGET cross-target builds are unsupported")
    if any(environ.get(key) for key in ("RUSTC", "RUSTDOC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTDOC", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER")):
        raise VerificationError("compiler and compiler-wrapper environment overrides are unsupported")
    exact = {
        "PATH", "PATHEXT", "HOME", "USERPROFILE", "CARGO_HOME", "RUSTUP_HOME",
        "CC", "CXX", "AR", "RANLIB", "LD", "CFLAGS", "CXXFLAGS", "LDFLAGS",
        "LIB", "LIBPATH", "INCLUDE", "SDKROOT", "MACOSX_DEPLOYMENT_TARGET",
        "SOURCE_DATE_EPOCH", "TEMP", "TMP", "TMPDIR",
    }
    secret = ("TOKEN", "PASSWORD", "SECRET", "CREDENTIAL", "AUTHORIZATION", "PRIVATE_KEY")
    return {
        key: hashlib.sha256(value.encode("utf8")).hexdigest()
        for key, value in sorted(environ.items())
        if (key in exact or key.startswith(("CARGO_", "RUST", "CC_", "CXX_", "AR_", "CFLAGS_", "CXXFLAGS_")))
        and not any(marker in key.upper() for marker in secret)
    }


def capture(command, cwd, env=None):
    """Capture command output without copying potential credential-bearing stderr."""
    result = subprocess.run(command, cwd=cwd, env=env, capture_output=True, text=True, check=False, timeout=1800)
    if result.returncode:
        raise VerificationError("build/toolchain command failed; no success record written")
    return result.stdout.strip()


def execute(command, cwd, env=None):
    result = subprocess.run(command, cwd=cwd, env=env, capture_output=True, text=True, check=False, timeout=600)
    if result.returncode:
        raise VerificationError("external evaluation failed; partial output is retained without a success record")


def context(root, toolchain):
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", toolchain):
        raise VerificationError("use an explicit numeric Rust toolchain, such as 1.85.0")
    compiler = capture(["rustc", "+" + toolchain, "-Vv"], root)
    cargo = capture(["cargo", "+" + toolchain, "-Vv"], root)
    if "release: " + toolchain not in compiler.splitlines():
        raise VerificationError("actual rustc release differs from the requested toolchain")
    if not cargo.splitlines() or not cargo.splitlines()[0].startswith("cargo " + toolchain + " "):
        raise VerificationError("actual Cargo version differs from the requested toolchain")
    return {
        "input_sha256": input_inventory(root),
        "cargo_config_sha256": cargo_config_inventory(root),
        "build_environment_sha256": build_environment(),
        "compiler": compiler,
        "cargo": cargo,
    }


def _canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("utf8")


def _seal(record):
    return hashlib.sha256(_canonical({key: value for key, value in record.items() if key != "record_sha256"})).hexdigest()


def write_new(path, record):
    record["record_sha256"] = _seal(record)
    with path.open("x", encoding="utf8", newline="\n") as stream:
        json.dump(record, stream, indent=2, sort_keys=True)
        stream.write("\n")


def _load(path):
    path = _regular_file(path)
    if path.stat().st_size > MAX_RECORD_BYTES:
        raise VerificationError("build record exceeds its size limit")

    def no_duplicates(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise VerificationError("duplicate build record key")
            result[key] = value
        return result

    value = json.loads(path.read_text(encoding="utf8"), object_pairs_hook=no_duplicates)
    if not isinstance(value, dict) or value.get("record_sha256") != _seal(value):
        raise VerificationError("build record checksum differs")
    return value


def _within(path, parent):
    return path != parent and path.is_relative_to(parent)


def _destination(path, root, build_directory=False):
    path = pathlib.Path(path)
    path = _checked_path(path if path.is_absolute() else root / path)
    allowed = [root / "target"]
    if not build_directory:
        allowed.append(root / "evaluation/results")
    if not any(_within(path, parent) for parent in allowed):
        raise VerificationError("destination must be below the allowed repository directory")
    return path


def _fresh(path):
    if path.exists():
        raise VerificationError("use a fresh directory; existing or partial outputs cannot be reused")
    path.mkdir(parents=True, exist_ok=False)


def _command(toolchain, build_dir):
    return ["cargo", "+" + toolchain, "build", "--locked", "--offline", "--release", "-p", "fieldkin-eval", "--bin", "fieldkin-eval", "--target-dir", str(build_dir / "cargo-target"), "--message-format=json"]


def _utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def build(build_dir, toolchain, root=None):
    """Build in a never-used target directory and seal the copied executable."""
    root = _checked_path(_root(root))
    build_dir = _destination(build_dir, root, build_directory=True)
    _fresh(build_dir)
    before = context(root, toolchain)
    command = _command(toolchain, build_dir)
    started = _utc()
    raw = capture(command, root)
    binaries = []
    for line in raw.splitlines():
        message = json.loads(line)
        target = message.get("target", {})
        if message.get("reason") == "compiler-artifact" and target.get("name") == "fieldkin-eval" and "bin" in target.get("kind", []) and message.get("executable"):
            binaries.append(pathlib.Path(message["executable"]))
    if len(binaries) != 1:
        raise VerificationError("Cargo did not report one evaluator executable")
    original = _regular_file(binaries[0])
    if not _within(original, build_dir / "cargo-target"):
        raise VerificationError("Cargo executable is outside the fresh isolated target directory")
    binary_sha = raw_sha256(original)
    bin_dir = build_dir / "bin"
    bin_dir.mkdir()
    binary = bin_dir / ("fieldkin-eval.exe" if original.suffix.lower() == ".exe" else "fieldkin-eval")
    shutil.copy2(original, binary)
    if raw_sha256(binary) != binary_sha or raw_sha256(original) != binary_sha:
        raise VerificationError("executable changed while being copied")
    after = context(root, toolchain)
    if after != before:
        raise VerificationError("inputs, compiler, config or build environment changed during build")
    record = {
        "protocol": PROTOCOL,
        "repository_root": str(root),
        "descriptive_git_head": capture(["git", "rev-parse", "HEAD"], root),
        "toolchain": toolchain,
        "command": command,
        "context_before": before,
        "context_after": after,
        "cargo_executable": str(original),
        "binary_path": binary.relative_to(build_dir).as_posix(),
        "binary_sha256": binary_sha,
        "started_utc": started,
        "completed_utc": _utc(),
        "trust_boundary": TRUST_BOUNDARY,
    }
    write_new(build_dir / BUILD_RECORD, record)
    return record


def _verified_build(build_dir, root):
    record = _load(build_dir / BUILD_RECORD)
    expected_keys = {"protocol", "repository_root", "descriptive_git_head", "toolchain", "command", "context_before", "context_after", "cargo_executable", "binary_path", "binary_sha256", "started_utc", "completed_utc", "trust_boundary", "record_sha256"}
    if set(record) != expected_keys or record["protocol"] != PROTOCOL or record["repository_root"] != str(root) or record["trust_boundary"] != TRUST_BOUNDARY:
        raise VerificationError("unexpected build record protocol or shape")
    if record["command"] != _command(record["toolchain"], build_dir):
        raise VerificationError("build command differs from the fixed protocol")
    if record["binary_path"] not in ("bin/fieldkin-eval", "bin/fieldkin-eval.exe"):
        raise VerificationError("unexpected evaluator binary path")
    original = _checked_path(record["cargo_executable"])
    if not _within(original, build_dir / "cargo-target"):
        raise VerificationError("recorded Cargo executable escaped the build directory")
    binary = _regular_file(build_dir / record["binary_path"])
    if raw_sha256(binary) != record["binary_sha256"]:
        raise VerificationError("copied evaluator executable changed")
    current = context(root, record["toolchain"])
    if record["context_before"] != record["context_after"] or record["context_before"] != current:
        raise VerificationError("source, data, compiler, Cargo config or environment differs from the build")
    return record, binary


def _artifact_hashes(output):
    if {path.name for path in output.iterdir()} != {"artifacts"}:
        raise VerificationError("unexpected files appeared in the evaluation output directory")
    directory = _checked_path(output / "artifacts")
    if not directory.is_dir() or {path.name for path in directory.iterdir()} != set(ARTIFACTS):
        raise VerificationError("external evaluator must produce exactly the three development artifacts")
    result = {}
    for name in ARTIFACTS:
        path = _regular_file(directory / name)
        if path.stat().st_size > MAX_ARTIFACT_BYTES:
            raise VerificationError("external artifact exceeds its size limit")
        result[name] = raw_sha256(path)
    return result


def run(build_dir, output, root=None):
    """Run only external development evaluation; never accept evaluator switches."""
    root = _checked_path(_root(root))
    build_dir = _destination(build_dir, root, build_directory=True)
    output = _destination(output, root)
    if output == build_dir or output.is_relative_to(build_dir) or build_dir.is_relative_to(output):
        raise VerificationError("build and output directories must not overlap")
    _fresh(output)
    record_path = build_dir / BUILD_RECORD
    record_sha = raw_sha256(record_path)
    record, binary = _verified_build(build_dir, root)
    command = [str(binary), "--external", "--output", str(output / "artifacts")]
    started = _utc()
    execute(command, root)
    after, checked_binary = _verified_build(build_dir, root)
    if after != record or checked_binary != binary or raw_sha256(record_path) != record_sha:
        raise VerificationError("build record or executable changed during evaluation")
    artifacts = _artifact_hashes(output)
    # Rehash after inspecting output so a failed post-check never creates success.
    if raw_sha256(binary) != record["binary_sha256"] or context(root, record["toolchain"]) != record["context_before"] or raw_sha256(record_path) != record_sha:
        raise VerificationError("verified context changed while inspecting artifacts")
    result = {
        "protocol": PROTOCOL,
        "status": "success",
        "partition": "development",
        "build_record": str(record_path),
        "build_record_sha256": record_sha,
        "binary_sha256": record["binary_sha256"],
        "input_sha256": record["context_before"]["input_sha256"],
        "command": command,
        "artifact_sha256": artifacts,
        "started_utc": started,
        "completed_utc": _utc(),
        "trust_boundary": TRUST_BOUNDARY,
    }
    write_new(output / RUN_RECORD, result)
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    build_parser = commands.add_parser("build")
    build_parser.add_argument("--build-dir", required=True, type=pathlib.Path)
    build_parser.add_argument("--toolchain", required=True)
    run_parser = commands.add_parser("run")
    run_parser.add_argument("--build-dir", required=True, type=pathlib.Path)
    run_parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        if args.action == "build":
            build(args.build_dir, args.toolchain)
            print("Verified external build recorded. Run evaluation into a fresh output directory.")
        else:
            run(args.build_dir, args.output)
            print("Verified external development evaluation recorded; reserved classes were not scored.")
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        # Avoid echoing subprocess output or arbitrary environment values.
        detail = str(error) if isinstance(error, VerificationError) else type(error).__name__
        print("Verified evaluation failed: " + detail + ". No success record was created for the failed action.", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
