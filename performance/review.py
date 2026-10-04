#!/usr/bin/env python3
"""Build separately, then measure five fixed caller-review release processes."""
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
import statistics
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PROTOCOL = "review-cost-v1"
POLICIES = ["none", "empty", "mixed-review"]
ITERATIONS = {16: 30, 64: 10, 128: 5}
RUNS = 5
PREAMBLE = [
    "fieldkin fixed caller-review benchmark; 16 samples/field when enabled",
    "one warm-up; input/engine/constraint construction excluded; report destruction included",
]
HEADER = ["policy", "fields_per_side", "samples", "one_to_one", "iterations", "total_ms", "us_per_match"]


def capture(args):
    return subprocess.run(args, cwd=ROOT, check=True, capture_output=True, text=True).stdout.strip()


def digest(path, binary=False):
    value = path.read_bytes()
    return hashlib.sha256(value if binary else value.replace(b"\r\n", b"\n")).hexdigest()


def sources():
    paths = [ROOT / name for name in ["Cargo.toml", "Cargo.lock", "examples/review_cost.rs", "performance/review.py"]]
    return {path.relative_to(ROOT).as_posix(): digest(path)
            for path in paths + sorted((ROOT / "src").rglob("*.rs"))}


def write_new(path, value):
    with path.open("x", encoding="utf8") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")


def environment():
    return {key: value for key, value in sorted(os.environ.items())
            if key in {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_TARGET"}
            or key.startswith(("CARGO_PROFILE_RELEASE_", "CARGO_PROFILE_BENCH_"))}


def validate(raw, smoke=False):
    lines = raw.splitlines()
    if lines[:2] != PREAMBLE:
        raise ValueError("unexpected benchmark preamble")
    text = "\n".join(lines[2:]) + "\n"
    reader = csv.DictReader(io.StringIO(text))
    if reader.fieldnames != HEADER:
        raise ValueError("unexpected benchmark CSV columns")
    rows = list(reader)
    keys = []
    for row in rows:
        if set(row) != set(HEADER) or any(value is None for value in row.values()):
            raise ValueError("malformed benchmark row")
        size = int(row["fields_per_side"])
        key = (row["policy"], size, row["samples"], row["one_to_one"])
        keys.append(key)
        if int(row["iterations"]) != (1 if smoke else ITERATIONS.get(size)):
            raise ValueError("unexpected benchmark iteration count")
        for name in ["total_ms", "us_per_match"]:
            value = float(row[name])
            if not math.isfinite(value) or value < 0:
                raise ValueError("invalid benchmark time")
    expected = [(policy, size, samples, assignment)
                for size in [16, 64, 128] for samples in ["false", "true"]
                for assignment in ["false", "true"] for policy in POLICIES]
    if keys != expected:
        raise ValueError("unexpected, duplicate or reordered benchmark workloads")
    return text, rows, keys


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["build", "run"])
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args()
    out = args.output.resolve()
    compiler = capture(["rustc", "+1.85.0", "-Vv"])
    flags = environment()
    if args.action == "build":
        out.mkdir(parents=True, exist_ok=True)
        if (out / "build.json").exists():
            raise ValueError("use a fresh output directory")
        before = sources()
        cmd = ["cargo", "+1.85.0", "build", "--release", "--locked", "-p", "fieldkin", "--example", "review_cost", "--message-format=json"]
        output = capture(cmd)
        binaries = [record["executable"] for line in output.splitlines()
                    if (record := json.loads(line)).get("reason") == "compiler-artifact"
                    and record.get("target", {}).get("name") == "review_cost"
                    and "example" in record.get("target", {}).get("kind", [])
                    and record.get("executable")]
        if len(binaries) != 1 or sources() != before:
            raise ValueError("source changed or Cargo did not identify a unique example binary")
        binary = pathlib.Path(binaries[0])
        write_new(out / "build.json", {
            "protocol": PROTOCOL, "commit": capture(["git", "rev-parse", "HEAD"]),
            "source_sha256": before, "binary_path": str(binary), "binary_sha256": digest(binary, True),
            "compiler": compiler, "command": cmd, "build_environment": flags,
            "platform": platform.platform(), "machine": platform.machine(),
            "processor": platform.processor(), "logical_cpus": os.cpu_count(),
            "python": platform.python_version(), "runs": RUNS,
            "built_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "cli": [sys.executable, str(pathlib.Path(__file__).resolve()), *sys.argv[1:]],
            "measurement_command": [str(binary)],
            "workloads": 36, "warmup_calls": 1,
            "iterations_by_size": {str(key): value for key, value in ITERATIONS.items()},
            "timing_scope": "matching and returned-report destruction; excludes input, engine and constraint construction",
            "order": "fixed sizes 16,64,128; missing then sampled; independent then assignment; none,empty,mixed-review",
            "mixed_review": "confirm first quarter to matching target IDs, explicitly keep second quarter unmatched, forbid one wrong cyclic target within remaining half per remaining source",
            "config": "default built-in weights and threshold 0.7; one_to_one per workload; global diagnostics disabled",
        })
        print("Build complete. Stop concurrent builds/tests before the measurement phase.", flush=True)
        return
    meta = json.loads((out / "build.json").read_text(encoding="utf8"))
    binary = pathlib.Path(meta["binary_path"])
    if list(out.glob("run-*.csv")) or (out / "summary.json").exists():
        raise ValueError("measurement output already exists")
    if meta["protocol"] != PROTOCOL or meta["runs"] != RUNS or meta["workloads"] != 36:
        raise ValueError("unexpected build protocol")
    if meta["source_sha256"] != sources() or meta["binary_sha256"] != digest(binary, True) or meta["compiler"] != compiler or meta["build_environment"] != flags:
        raise ValueError("sources, binary, compiler or build environment changed")
    grouped = {}
    execution = []
    for run in range(1, RUNS + 1):
        started = datetime.datetime.now(datetime.timezone.utc).isoformat()
        cmd = [str(binary)]
        raw = capture(cmd)
        text, rows, keys = validate(raw)
        for key, row in zip(keys, rows):
            grouped.setdefault(key, []).append(float(row["us_per_match"]))
        filename = f"run-{run}.csv"
        with (out / filename).open("x", encoding="utf8") as stream:
            stream.write(text)
        execution.append({"run": run, "command": cmd, "started_utc": started, "file": filename})
        print(f"Review cost process {run}/{RUNS} completed", flush=True)
    if meta["source_sha256"] != sources() or meta["binary_sha256"] != digest(binary, True) or environment() != flags:
        raise ValueError("sources, binary or build environment changed during measurement")
    rows = [{"policy": policy, "fields_per_side": size, "samples": samples == "true", "one_to_one": assignment == "true",
             "process_means_us": values, "median_us": statistics.median(values), "min_us": min(values), "max_us": max(values)}
            for (policy, size, samples, assignment), values in sorted(grouped.items())]
    write_new(out / "summary.json", {
        "protocol": PROTOCOL, "rows": rows, "execution": execution,
        "cli": [sys.executable, str(pathlib.Path(__file__).resolve()), *sys.argv[1:]],
        "interpretation": "Five process means after one warmup; fixed order, no CPU affinity, confidence intervals or independent real workloads. Mixed review changes eligibility, selection, residual assignment work and report construction while all pairs are still scored; this is cost characterization, not a pure optimization comparison.",
    })


if __name__ == "__main__":
    main()
