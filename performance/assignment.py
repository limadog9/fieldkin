#!/usr/bin/env python3
"""Fixed before/after assignment experiment. Build separately from measurement.

The local source/configuration and executable hashes detect accidental drift;
they are not a signature or hermetic-build attestation. No holdout is involved.
"""

import argparse
import csv
import datetime
import hashlib
import io
import json
import math
import os
import pathlib
import platform
import shutil
import statistics
import subprocess
import sys


TOOLCHAIN = "1.85.0"
PROTOCOL = "assignment-compaction-v1"
RUNS = 5
FAMILIES = ["dense-disabled", "dense-bounded", "sparse-complete", "half-excluded-disabled",
            "all-excluded-complete", "mixed-bounded", "reserved-complete", "wide-complete", "tall-bounded"]
ITERATIONS = {16: 10, 64: 4, 128: 2}
PREAMBLE = ["fieldkin fixed assignment comparison; no samples; all original pairs scored",
            "one warm-up; construction excluded; matching and report destruction timed"]
HEADER = ["family", "size", "sources", "targets", "iterations", "total_ms", "us_per_match",
          "proposed", "confirmed", "excluded", "unmatched_sources", "unmatched_targets",
          "status", "solves", "work", "alternatives", "objective"]
HARNESS = ["examples/assignment_cost.rs", "performance/assignment.py"]


def capture(args, root=None):
    return subprocess.run([str(arg) for arg in args], cwd=root, check=True,
                          capture_output=True, text=True).stdout.strip()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inventory(root):
    # Include every recursive library input, not just currently known .rs files.
    files = sorted(path for path in (root / "src").rglob("*") if path.is_file())
    files += [root / name for name in ["Cargo.toml", "Cargo.lock", "README.md", "evaluation/Cargo.toml", *HARNESS]]
    for directory in [root, root / "evaluation"]:
        files += [directory / name for name in ["build.rs", "rust-toolchain", "rust-toolchain.toml"]
                  if (directory / name).exists()]
    return dict(sorted((path.relative_to(root).as_posix(), digest(path)) for path in files))


def cargo_configs(root):
    cargo_dir = pathlib.Path(os.environ.get("CARGO_HOME", pathlib.Path.home() / ".cargo"))
    if not cargo_dir.is_absolute():
        cargo_dir = root / cargo_dir
    directories = {directory / ".cargo" for directory in [root, *root.parents]} | {cargo_dir}
    return {str(path.resolve()): digest(path) for directory in sorted(directories)
            for name in ["config", "config.toml"] if (path := directory / name).exists()}


def environment():
    names = {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER",
             "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_TARGET", "CARGO_HOME", "RUSTUP_HOME",
             "RUSTUP_TOOLCHAIN", "RUSTDOCFLAGS", "CARGO_ENCODED_RUSTDOCFLAGS"}
    return {key: value for key, value in sorted(os.environ.items())
            if key in names or key.startswith(("CARGO_PROFILE_RELEASE_", "CARGO_PROFILE_BENCH_", "CARGO_TARGET_"))}


def write_new(path, value):
    with path.open("x", encoding="utf8") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")


def compiler():
    return {name: capture([name, "+" + TOOLCHAIN, "-Vv"]) for name in ["rustc", "cargo"]}


def machine():
    return {"platform": platform.platform(), "machine": platform.machine(),
            "processor": platform.processor(), "logical_cpus": os.cpu_count(),
            "python": platform.python_version()}


def validate(raw, smoke=False):
    lines = raw.splitlines()
    if lines[:2] != PREAMBLE:
        raise ValueError("unexpected benchmark preamble")
    text = "\n".join(lines[2:]) + "\n"
    reader = csv.DictReader(io.StringIO(text))
    if reader.fieldnames != HEADER:
        raise ValueError("unexpected benchmark columns")
    rows = list(reader)
    keys, behavior = [], []
    for row in rows:
        if set(row) != set(HEADER) or any(value is None for value in row.values()):
            raise ValueError("malformed benchmark row")
        size, family = int(row["size"]), row["family"]
        keys.append((size, family))
        if size not in ITERATIONS or family not in FAMILIES:
            raise ValueError("unknown benchmark workload")
        n, m = (size // 4 if family == "wide-complete" else size), (size // 4 if family == "tall-bounded" else size)
        confirmed = size // 4 if family == "mixed-bounded" else size - 4 if family == "reserved-complete" else 0
        excluded = size if family == "all-excluded-complete" else size // 2 if family == "half-excluded-disabled" else size // 4 if family == "mixed-bounded" else 0
        proposed = {"dense-disabled": size, "dense-bounded": size, "sparse-complete": 4,
                    "half-excluded-disabled": size // 2, "all-excluded-complete": 0,
                    "mixed-bounded": size // 2, "reserved-complete": 4,
                    "wide-complete": 4, "tall-bounded": 4}[family]
        solves = 0 if family.endswith("disabled") else 2 if family.endswith("bounded") else proposed
        status = "Disabled" if family.endswith("disabled") else "BudgetExhausted" if family.endswith("bounded") else "Complete"
        expected = {"sources": n, "targets": m, "iterations": 1 if smoke else ITERATIONS[size],
                    "proposed": proposed, "confirmed": confirmed, "excluded": excluded,
                    "unmatched_sources": n - proposed - confirmed, "unmatched_targets": m - proposed - confirmed,
                    "solves": solves, "work": solves * n * n * (m + n)}
        if any(int(row[key]) != value for key, value in expected.items()) or row["status"] != status:
            raise ValueError("unexpected selection counts, diagnostic status or original-dimension budget charge")
        if int(row["alternatives"]) < 0 or int(row["alternatives"]) > solves:
            raise ValueError("invalid alternatives count")
        if status == "Disabled":
            if row["objective"] != "none":
                raise ValueError("disabled diagnostic objective must be absent")
        elif not math.isclose(float(row["objective"]), 0.85 * proposed, rel_tol=1e-12, abs_tol=1e-12):
            raise ValueError("unexpected automatic-only objective")
        for key in ["total_ms", "us_per_match"]:
            value = float(row[key])
            if not math.isfinite(value) or value < 0:
                raise ValueError("invalid benchmark duration")
        if not math.isclose(float(row["total_ms"]) * 1000 / int(row["iterations"]),
                            float(row["us_per_match"]), rel_tol=1e-6, abs_tol=0.001):
            raise ValueError("inconsistent benchmark durations")
        behavior.append({key: row[key] for key in HEADER if key not in ["total_ms", "us_per_match"]})
    if keys != [(size, family) for size in ITERATIONS for family in FAMILIES]:
        raise ValueError("missing, duplicate or reordered benchmark workloads")
    return text, rows, behavior


def verify(meta):
    if digest(pathlib.Path(__file__)) != meta["checkouts"]["candidate"]["source_sha256"]["performance/assignment.py"]:
        raise ValueError("executing driver differs from the recorded candidate driver")
    if meta["protocol"] != PROTOCOL or meta["compiler"] != compiler() or meta["build_environment"] != environment():
        raise ValueError("protocol, compiler or environment changed after build")
    if meta["runs_per_revision"] != RUNS or meta["families"] != FAMILIES or meta["iterations"] != {str(k): v for k, v in ITERATIONS.items()}:
        raise ValueError("measurement protocol changed")
    for entry in meta["checkouts"].values():
        root = pathlib.Path(entry["path"])
        if inventory(root) != entry["source_sha256"] or cargo_configs(root) != entry["cargo_config_sha256"]:
            raise ValueError("source inventory or Cargo configuration changed")
        if digest(pathlib.Path(entry["binary_path"])) != entry["binary_sha256"]:
            raise ValueError("benchmark executable changed")


def build(args):
    baseline, candidate, out = args.baseline.resolve(), args.candidate.resolve(), args.output.resolve()
    if digest(pathlib.Path(__file__)) != digest(candidate / "performance/assignment.py"):
        raise ValueError("executing driver differs from the candidate driver")
    if baseline == candidate:
        raise ValueError("baseline and candidate must be separate checkouts")
    if out.exists() and any(out.iterdir()):
        raise ValueError("use an empty output directory")
    out.mkdir(parents=True, exist_ok=True)
    for relative in HARNESS:
        source, target = candidate / relative, baseline / relative
        if target.exists() and digest(target) != digest(source):
            raise ValueError("baseline already contains a different harness; use a fresh baseline checkout")
        if not target.exists():
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
    meta = {"protocol": PROTOCOL, "compiler": compiler(), "build_environment": environment(),
            "machine": machine(), "runs_per_revision": RUNS, "families": FAMILIES,
            "copied_harness_sha256": {name: digest(candidate / name) for name in HARNESS},
            "config": {"one_to_one": True, "abstain_on_ambiguity": False, "min_score": 0.70,
                       "ambiguity_margin": 0.08, "max_candidates": 5, "samples": False,
                       "max_work": 536870912, "objective_margin": 0.08,
                       "diagnostic_solves": {"disabled": 0, "bounded": 2, "complete": 128},
                       "signals": "dense-disabled uses default built-ins; all other cases use one fixed synthetic graph matcher at weight 1"},
            "iterations": ITERATIONS, "warmup_calls": 1, "post_timing_validation_calls": 1,
            "order": "fixed workload order; baseline,candidate on odd runs and candidate,baseline on even runs",
            "timing_scope": "matching and report destruction; excludes inputs, engine, constraints and behavior summarization",
            "built_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "cli": [sys.executable, str(pathlib.Path(__file__).resolve()), *sys.argv[1:]], "checkouts": {}}
    for revision, root in [("baseline", baseline), ("candidate", candidate)]:
        before, config = inventory(root), cargo_configs(root)
        target = root / "target" / "assignment-cost" / hashlib.sha256(str(out).encode()).hexdigest()[:16]
        target.mkdir(parents=True, exist_ok=False)
        cmd = ["cargo", "+" + TOOLCHAIN, "build", "--release", "--locked", "-p", "fieldkin",
               "--example", "assignment_cost", "--target-dir", str(target), "--message-format=json"]
        print(f"Building {revision}", flush=True)
        output = capture(cmd, root)
        binaries = [record["executable"] for line in output.splitlines()
                    if (record := json.loads(line)).get("reason") == "compiler-artifact"
                    and record.get("target", {}).get("name") == "assignment_cost"
                    and "example" in record.get("target", {}).get("kind", []) and record.get("executable")]
        if len(binaries) != 1 or before != inventory(root) or config != cargo_configs(root):
            raise ValueError("build inputs changed or Cargo did not report one example executable")
        binary = pathlib.Path(binaries[0])
        _, _, smoke_behavior = validate(capture([binary, "--smoke"], root), smoke=True)
        meta["checkouts"][revision] = {"path": str(root), "commit": capture(["git", "rev-parse", "HEAD"], root),
                                       "git_status": capture(["git", "status", "--short"], root),
                                       "source_sha256": before, "cargo_config_sha256": config,
                                       "binary_path": str(binary), "binary_sha256": digest(binary),
                                       "build_command": cmd, "smoke_behavior": smoke_behavior}
    if meta["checkouts"]["baseline"]["smoke_behavior"] != meta["checkouts"]["candidate"]["smoke_behavior"]:
        raise ValueError("baseline and candidate workload behavior differs")
    # JSON round-trip gives metadata the same representation used by the run command.
    verify(json.loads(json.dumps(meta)))
    write_new(out / "build.json", meta)
    print("Builds and workload checks passed. Stop concurrent builds/tests before measuring.", flush=True)


def measure(args):
    out = args.output.resolve()
    if set(path.name for path in out.iterdir()) != {"build.json"}:
        raise ValueError("measurement destination must contain only build.json")
    meta = json.loads((out / "build.json").read_text(encoding="utf8"))
    verify(meta)
    execution, grouped, reference = [], {}, None
    for run in range(1, RUNS + 1):
        for revision in (["baseline", "candidate"] if run % 2 else ["candidate", "baseline"]):
            entry = meta["checkouts"][revision]
            started = datetime.datetime.now(datetime.timezone.utc).isoformat()
            cmd = [entry["binary_path"]]
            text, rows, behavior = validate(capture(cmd, entry["path"]))
            smoke_behavior = [dict(row, iterations="1") for row in behavior]
            if smoke_behavior != entry["smoke_behavior"]:
                raise ValueError("workload behavior changed since the build smoke check")
            if reference is None:
                reference = behavior
            elif behavior != reference:
                raise ValueError("workload behavior changed across revisions or processes")
            verify(meta)
            filename = f"{revision}-run-{run}.csv"
            with (out / filename).open("x", encoding="utf8") as stream:
                stream.write(text)
            execution.append({"run": run, "revision": revision, "command": cmd,
                              "started_utc": started, "file": filename, "sha256": digest(out / filename)})
            for row in rows:
                grouped.setdefault((int(row["size"]), row["family"]), {}).setdefault(revision, []).append(float(row["us_per_match"]))
            print(f"Assignment process {run}/{RUNS}, {revision} completed", flush=True)
    verify(meta)
    rows = []
    for (size, family), revisions in sorted(grouped.items()):
        entry = {"size": size, "family": family}
        for revision, values in revisions.items():
            entry[revision] = {"process_means_us": values, "median_us": statistics.median(values),
                               "min_us": min(values), "max_us": max(values)}
        old, new = entry["baseline"]["median_us"], entry["candidate"]["median_us"]
        entry["median_change_percent"] = (new / old - 1) * 100 if old else None
        rows.append(entry)
    write_new(out / "summary.json", {"protocol": PROTOCOL, "execution": execution, "rows": rows,
              "behavior": reference, "machine": machine(),
              "cli": [sys.executable, str(pathlib.Path(__file__).resolve()), *sys.argv[1:]],
              "interpretation": "Five independent process means per revision, alternating revision order. Fixed synthetic graph shapes, no CPU affinity, confidence interval or semantic accuracy claim. All original pairs remain scored. Count/status equality is not full report equality; use separate differential qualification. Hashes bind the scoped local inputs and binaries, not a hermetic build or a trusted signature."})


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["build", "run"])
    parser.add_argument("--baseline", type=pathlib.Path)
    parser.add_argument("--candidate", type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args()
    if args.action == "build":
        if args.baseline is None or args.candidate is None:
            parser.error("build requires --baseline and --candidate")
        build(args)
    else:
        if args.baseline is not None or args.candidate is not None:
            parser.error("run uses the checkouts recorded in build.json")
        measure(args)


if __name__ == "__main__":
    main()
