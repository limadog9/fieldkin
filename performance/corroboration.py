#!/usr/bin/env python3
"""Build the fixed matching benchmark, then measure five separate release processes."""
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
    value = path.read_bytes()
    return hashlib.sha256(value if binary else value.replace(b"\r\n", b"\n")).hexdigest()


def sources():
    paths = [ROOT / name for name in ["Cargo.toml", "Cargo.lock", "benches/matching.rs", "performance/corroboration.py"]]
    return {path.relative_to(ROOT).as_posix(): digest(path)
            for path in paths + sorted((ROOT / "src").glob("*.rs"))}


def write_new(path, value):
    with path.open("x", encoding="utf8") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["build", "run"])
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args()
    out = args.output.resolve()
    compiler = capture(["rustc", "+1.85.0", "-Vv"])
    flags = {key: value for key, value in os.environ.items()
             if key in {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_TARGET"}
             or key.startswith(("CARGO_PROFILE_RELEASE_", "CARGO_PROFILE_BENCH_"))}
    if args.action == "build":
        out.mkdir(parents=True, exist_ok=True)
        if (out / "build.json").exists():
            raise ValueError("use a fresh output directory")
        before = sources()
        cmd = ["cargo", "+1.85.0", "bench", "--locked", "-p", "fieldkin", "--bench", "matching", "--no-run", "--message-format=json"]
        output = capture(cmd)
        binaries = [record["executable"] for line in output.splitlines()
                    if (record := json.loads(line)).get("reason") == "compiler-artifact"
                    and record.get("target", {}).get("name") == "matching" and record.get("executable")]
        if len(binaries) != 1 or sources() != before:
            raise ValueError("source changed or Cargo did not identify a unique benchmark binary")
        binary = pathlib.Path(binaries[0])
        write_new(out / "build.json", {
            "protocol": "corroboration-cost-v1", "commit": capture(["git", "rev-parse", "HEAD"]),
            "source_sha256": before, "binary_path": str(binary), "binary_sha256": digest(binary, True),
            "compiler": compiler, "command": cmd, "build_environment": flags,
            "platform": platform.platform(), "machine": platform.machine(),
            "processor": platform.processor(), "logical_cpus": os.cpu_count(), "runs": 5,
            "workloads": 36, "warmup_calls": 1, "iterations_by_size": {"16": 30, "64": 10, "128": 5},
            "timing_scope": "match_schemas and returned-report destruction; excludes input/engine construction",
            "order": "fixed sizes 16,64,128; missing then sampled; independent then assignment; combined,sample-supported,name-only",
        })
        return
    meta = json.loads((out / "build.json").read_text(encoding="utf8"))
    binary = pathlib.Path(meta["binary_path"])
    if list(out.glob("run-*.csv")) or (out / "summary.json").exists():
        raise ValueError("measurement output already exists")
    if meta["source_sha256"] != sources() or meta["binary_sha256"] != digest(binary, True) or meta["compiler"] != compiler or meta["build_environment"] != flags:
        raise ValueError("sources, binary, compiler or build environment changed")
    grouped = {}
    expected = {(model, size, samples, assignment)
                for model in ["combined", "sample-supported", "name-only"]
                for size in [16, 64, 128] for samples in ["false", "true"] for assignment in ["false", "true"]}
    for run in range(1, 6):
        raw = capture([str(binary)])
        lines = raw.splitlines()
        text = "\n".join(lines[2:]) + "\n"
        rows = list(csv.DictReader(io.StringIO(text)))
        keys = [(row["engine"], int(row["fields_per_side"]), row["samples"], row["one_to_one"]) for row in rows]
        if len(rows) != 36 or set(keys) != expected:
            raise ValueError("unexpected benchmark workload inventory")
        for key, row in zip(keys, rows):
            grouped.setdefault(key, []).append(float(row["us_per_match"]))
        with (out / f"run-{run}.csv").open("x", encoding="utf8") as stream:
            stream.write(text)
        print(f"Corroboration cost process {run}/5 completed", flush=True)
    if meta["source_sha256"] != sources() or meta["binary_sha256"] != digest(binary, True):
        raise ValueError("sources or binary changed during measurement")
    rows = [{"model": model, "fields_per_side": size, "samples": samples == "true", "one_to_one": assignment == "true",
             "process_means_us": values, "median_us": statistics.median(values), "min_us": min(values), "max_us": max(values)}
            for (model, size, samples, assignment), values in sorted(grouped.items())]
    write_new(out / "summary.json", {"protocol": "corroboration-cost-v1", "rows": rows,
        "interpretation": "Five process means after one warmup; fixed order, no CPU affinity, confidence intervals or independent real workloads. Enabled gating changes eligibility and report construction; this is cost characterization, not a pure optimization comparison."})


if __name__ == "__main__":
    main()
