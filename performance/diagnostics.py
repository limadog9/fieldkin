#!/usr/bin/env python3
"""Freeze a release diagnostics-cost binary, then measure five independent processes."""
import argparse
import csv
import hashlib
import io
import json
import os
import pathlib
import platform
import statistics
import subprocess

ROOT = pathlib.Path(__file__).resolve().parent.parent

def capture(args):
    return subprocess.run(args, cwd=ROOT, check=True, capture_output=True, text=True).stdout.strip()

def digest(path, binary=False):
    data = path.read_bytes()
    return hashlib.sha256(data if binary else data.replace(b"\r\n", b"\n")).hexdigest()

def sources():
    paths = [ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "examples/diagnostics_cost.rs",
             ROOT / "performance/diagnostics.py"] + sorted((ROOT / "src").glob("*.rs"))
    return {p.relative_to(ROOT).as_posix(): digest(p) for p in paths}

def write_new(path, data):
    with path.open("x", encoding="utf8") as stream:
        json.dump(data, stream, indent=2, sort_keys=True)
        stream.write("\n")

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["build", "run"])
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    compiler = capture(["rustc", "+1.85.0", "-Vv"])
    flags = {k:v for k,v in os.environ.items() if k in {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_TARGET"} or k.startswith("CARGO_PROFILE_RELEASE_")}
    if args.action == "build":
        out.mkdir(parents=True, exist_ok=True)
        if (out / "build.json").exists():
            raise ValueError("use a fresh output directory")
        before = sources()
        cmd = ["cargo", "+1.85.0", "build", "--release", "--locked", "--example", "diagnostics_cost", "--message-format=json"]
        output = capture(cmd)
        executables = [record["executable"] for line in output.splitlines()
            if (record := json.loads(line)).get("reason") == "compiler-artifact"
            and record.get("target", {}).get("name") == "diagnostics_cost" and record.get("executable")]
        if len(executables) != 1 or before != sources():
            raise ValueError("source changed during build or executable was not uniquely identified")
        binary = pathlib.Path(executables[0])
        write_new(out / "build.json", {"protocol":"diagnostics-cost-v1", "commit":capture(["git","rev-parse","HEAD"]),
            "source_sha256":sources(), "compiler":compiler, "command":cmd, "binary_sha256":digest(binary, True),
            "binary_path":str(binary),
            "build_environment":flags, "platform":platform.platform(), "machine":platform.machine(),
            "processor":platform.processor(), "logical_cpus":os.cpu_count(), "runs":5,
            "timing_scope":"match_schemas call only; excludes input/engine setup, assertion checks and destruction of returned report",
            "warmup_per_workload":1, "measured_calls_per_workload":3})
        return
    meta = json.loads((out / "build.json").read_text(encoding="utf8"))
    binary = pathlib.Path(meta["binary_path"])
    if list(out.glob("run-*.csv")) or (out / "summary.json").exists():
        raise ValueError("measurement output already exists")
    if meta["source_sha256"] != sources() or meta["binary_sha256"] != digest(binary, True) or meta["compiler"] != compiler or meta["build_environment"] != flags:
        raise ValueError("sources, binary or build environment changed")
    grouped = {}
    for run in range(1, 6):
        text = capture([str(binary)]) + "\n"
        rows = list(csv.DictReader(io.StringIO(text)))
        if len(rows) != 60 or any(r["debug_assertions"] != "false" for r in rows):
            raise ValueError("unexpected cost workload count or debug binary")
        (out / f"run-{run}.csv").write_text(text, encoding="utf8")
        per_process = {}
        for row in rows:
            key = (int(row["source_count"]), int(row["target_count"]), row["mode"])
            per_process.setdefault(key, []).append(int(row["elapsed_ns"]))
        for key, values in per_process.items():
            if len(values) != 3:
                raise ValueError("expected three measurements per workload")
            grouped.setdefault(key, []).append(statistics.mean(values))
        print(f"Diagnostics cost process {run}/5 completed", flush=True)
    if meta["source_sha256"] != sources() or meta["binary_sha256"] != digest(binary, True):
        raise ValueError("sources or binary changed during measurement")
    rows = [{"source_count":s,"target_count":t,"mode":mode,"process_means_ns":values,
             "median_ns":statistics.median(values),"min_ns":min(values),"max_ns":max(values)}
            for (s,t,mode), values in sorted(grouped.items())]
    write_new(out / "summary.json", {"protocol":"diagnostics-cost-v1", "rows":rows,
        "interpretation":"Five process means, each from three calls after one warmup; same-machine descriptive timings, no confidence interval or CPU affinity."})

if __name__ == "__main__":
    main()
