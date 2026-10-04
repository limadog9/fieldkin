#!/usr/bin/env python3
"""Run the frozen Northix task using a shared verified evaluator build.

Build with evaluation/verified.py build. This wrapper adds a fixed --northix
execution route, never arbitrary evaluator arguments. Score-file semantics and
approved mode/producer identities are checked by the Rust evaluator. No holdout
is scored. Python 3.11+, standard library only.
"""

import argparse
import pathlib
import subprocess
import sys

import verified


PROTOCOL = "fieldkin-northix-verified-v1"
ARTIFACTS = ("results.json", "predictions.jsonl", "results.md")
MAX_SCORE_BYTES = 16 * 1024 * 1024


def score_inventory(paths, root):
    """Validate score paths and record exact bytes, without interpreting labels."""
    if len(paths) not in (0, 2):
        raise verified.VerificationError("Northix accepts zero or two approved-mode score files")
    result = []
    seen = set()
    for path in paths:
        path = pathlib.Path(path)
        path = verified._regular_file(path if path.is_absolute() else root / path)
        if path in seen:
            raise verified.VerificationError("Northix score files must use distinct paths")
        seen.add(path)
        size = path.stat().st_size
        if size > MAX_SCORE_BYTES:
            raise verified.VerificationError("Northix score input exceeds its byte limit")
        result.append({"path": str(path), "bytes": size, "sha256": verified.raw_sha256(path)})
    return result


def artifact_hashes(output):
    """Require only the three complete, bounded Northix output artifacts."""
    if {path.name for path in output.iterdir()} != {"artifacts"}:
        raise verified.VerificationError("unexpected files appeared in the Northix output directory")
    directory = verified._checked_path(output / "artifacts")
    if not directory.is_dir() or {path.name for path in directory.iterdir()} != set(ARTIFACTS):
        raise verified.VerificationError("Northix must produce exactly the three expected artifacts")
    result = {}
    for name in ARTIFACTS:
        path = verified._regular_file(directory / name)
        if path.stat().st_size > verified.MAX_ARTIFACT_BYTES:
            raise verified.VerificationError("Northix artifact exceeds its byte limit")
        result[name] = verified.raw_sha256(path)
    return result


def run(build_dir, output, scores=(), root=None):
    """Execute the fixed task and seal success only after every post-run guard."""
    root = verified._checked_path(verified._root(root))
    build_dir = verified._destination(build_dir, root, build_directory=True)
    output = verified._destination(output, root)
    if output == build_dir or output.is_relative_to(build_dir) or build_dir.is_relative_to(output):
        raise verified.VerificationError("build and output directories must not overlap")
    before_scores = score_inventory(scores, root)
    verified._fresh(output)
    # Deliberately do not create output/artifacts: Northix creates it exclusively.
    record_path = build_dir / verified.BUILD_RECORD
    record_sha = verified.raw_sha256(record_path)
    record, binary = verified._verified_build(build_dir, root)
    command = [str(binary), "--northix", "--output", str(output / "artifacts")]
    for score in before_scores:
        command += ["--scores", score["path"]]
    started = verified._utc()
    verified.execute(command, root)
    after, checked_binary = verified._verified_build(build_dir, root)
    if after != record or checked_binary != binary or verified.raw_sha256(record_path) != record_sha:
        raise verified.VerificationError("build record or executable changed during Northix evaluation")
    if score_inventory(scores, root) != before_scores:
        raise verified.VerificationError("Northix score inputs changed during evaluation")
    artifacts = artifact_hashes(output)
    if (verified.raw_sha256(binary) != record["binary_sha256"]
            or verified.context(root, record["toolchain"]) != record["context_before"]
            or verified.raw_sha256(record_path) != record_sha
            or score_inventory(scores, root) != before_scores):
        raise verified.VerificationError("verified context or scores changed while inspecting Northix artifacts")
    result = {
        "protocol": PROTOCOL, "status": "success", "task": "northix-class-equivalence",
        "partition": "fixed-external-diagnostic", "reserved_holdouts_scored": False,
        "build_record": str(record_path), "build_record_sha256": record_sha,
        "binary_sha256": record["binary_sha256"],
        "input_sha256": record["context_before"]["input_sha256"],
        "score_files": before_scores, "command": command, "artifact_sha256": artifacts,
        "started_utc": started, "completed_utc": verified._utc(),
        "trust_boundary": verified.TRUST_BOUNDARY,
    }
    verified.write_new(output / verified.RUN_RECORD, result)
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-dir", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    parser.add_argument("--scores", action="append", default=[], type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        run(args.build_dir, args.output, args.scores)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        detail = str(error) if isinstance(error, verified.VerificationError) else type(error).__name__
        print("Verified Northix evaluation failed: " + detail + ". No success record was created for the failed action.", file=sys.stderr)
        return 1
    print("Verified Northix diagnostic recorded; reserved holdouts were not scored.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
