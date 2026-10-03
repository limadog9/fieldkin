#!/usr/bin/env python3
"""Build first; measure later without concurrent compiler activity. Python 3.9+."""

import argparse
import datetime
import hashlib
import json
import os
import pathlib
import platform
import shutil
import statistics
import subprocess
import sys


TOOLCHAIN = "1.85.0"
RUNS = 5


def build_environment():
    names = {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_TARGET"}
    return {name: value for name, value in sorted(os.environ.items())
            if name in names or name.startswith("CARGO_PROFILE_RELEASE_")}


def command(args, cwd=None):
    completed = subprocess.run(
        [str(arg) for arg in args], cwd=cwd, check=True, capture_output=True, text=True
    )
    return completed.stdout.strip()


def digest(path):
    return hashlib.sha256(path.read_bytes().replace(b"\r\n", b"\n")).hexdigest()


def binary_digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources(root):
    files = [root / "Cargo.toml", root / "Cargo.lock"]
    files += sorted((root / "src").glob("*.rs"))
    files += [root / "performance" / name for name in ["Cargo.toml", "Cargo.lock", "run.py"]]
    files += sorted((root / "performance" / "src").glob("*.rs"))
    return {str(path.relative_to(root)).replace("\\", "/"): digest(path) for path in files}


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def machine():
    record = {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "processor": platform.processor(),
        "logical_cpus": os.cpu_count(),
        "python": platform.python_version(),
    }
    if sys.platform == "win32":
        record["cpu"] = command([
            "powershell", "-NoProfile", "-Command",
            "Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors | ConvertTo-Json -Compress",
        ])
    elif pathlib.Path("/proc/cpuinfo").exists():
        record["cpu"] = next(
            (line.partition(":")[2].strip() for line in pathlib.Path("/proc/cpuinfo").read_text().splitlines()
             if line.startswith("model name")), "unknown"
        )
    return record


def build(args):
    candidate = pathlib.Path(args.candidate).resolve()
    baseline = pathlib.Path(args.baseline).resolve()
    output = pathlib.Path(args.output).resolve()
    if candidate == baseline:
        raise ValueError("baseline and candidate must be separate checkouts")
    if (output / "build.json").exists():
        raise ValueError("build.json already exists; choose a fresh output directory")
    output.mkdir(parents=True, exist_ok=True)
    # Only known harness files are copied; no deletion and no library-source mutation.
    for relative in ["Cargo.toml", "Cargo.lock", "run.py", "README.md", ".gitignore", "src/main.rs"]:
        destination = baseline / "performance" / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(candidate / "performance" / relative, destination)
    metadata = {
        "protocol": "fieldkin-stage2-v1",
        "compiler": command(["rustc", "+" + TOOLCHAIN, "-Vv"]),
        "machine": machine(),
        "build_environment": build_environment(),
        "runs_per_revision": RUNS,
        "warmup_calls": 3,
        "allocations_warmup_calls": 1,
        "timing_instrumented": False,
        "revision_order": "baseline,candidate on odd runs; candidate,baseline on even runs",
        "built_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "checkouts": {},
    }
    for revision, root in [("baseline", baseline), ("candidate", candidate)]:
        entry = {
            "path": str(root),
            "commit": command(["git", "rev-parse", "HEAD"], root),
            "git_status": command(["git", "status", "--short"], root),
            "source_sha256": sources(root),
            "binaries": {},
        }
        for mode in ["timing", "allocations"]:
            target = root / "performance" / "target" / mode
            cmd = [
                "cargo", "+" + TOOLCHAIN, "build", "--release", "--locked",
                "--manifest-path", root / "performance/Cargo.toml", "--target-dir", target,
            ]
            if mode == "allocations":
                cmd += ["--features", "allocations"]
            print(f"Building {revision}/{mode}", flush=True)
            subprocess.run([str(part) for part in cmd], check=True)
            executable = target / "release" / ("fieldkin-performance.exe" if os.name == "nt" else "fieldkin-performance")
            entry["binaries"][mode] = {
                "path": str(executable), "sha256": binary_digest(executable),
                "command": [str(part) for part in cmd],
            }
        metadata["checkouts"][revision] = entry
    write_json(output / "build.json", metadata)
    print("Builds complete. Stop concurrent builds/tests before running the measurement phase.", flush=True)


def measure(args):
    output = pathlib.Path(args.output).resolve()
    metadata = json.loads((output / "build.json").read_text(encoding="utf-8"))
    if (output / "run.json").exists() or list(output.glob("*-run-*.jsonl")):
        raise ValueError("measurement artifacts already exist; choose a fresh build/output directory")
    if command(["rustc", "+" + TOOLCHAIN, "-Vv"]) != metadata["compiler"]:
        raise ValueError("compiler changed after build")
    if build_environment() != metadata["build_environment"]:
        raise ValueError("build environment changed after build")
    for entry in metadata["checkouts"].values():
        if sources(pathlib.Path(entry["path"])) != entry["source_sha256"]:
            raise ValueError("measured sources changed after build; rebuild into a fresh output directory")
        for executable in entry["binaries"].values():
            if binary_digest(pathlib.Path(executable["path"])) != executable["sha256"]:
                raise ValueError("binary changed after build")
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    execution = []
    for mode in ["timing", "allocations"]:
        for run in range(1, RUNS + 1):
            revisions = ["baseline", "candidate"] if run % 2 else ["candidate", "baseline"]
            for revision in revisions:
                binary = metadata["checkouts"][revision]["binaries"][mode]["path"]
                cmd = [binary] + (["--smoke"] if mode == "allocations" else [])
                print(f"Measuring {mode}, independent process {run}/{RUNS}, {revision}", flush=True)
                stamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
                raw = command(cmd)
                rows = [json.loads(line) for line in raw.splitlines()]
                if len(rows) != 50 or len({row["workload"] for row in rows}) != 50:
                    raise ValueError("unexpected or duplicate workload output")
                filename = f"{mode}-{revision}-run-{run}.jsonl"
                (output / filename).write_text(raw + "\n", encoding="utf-8")
                execution.append({"mode": mode, "revision": revision, "run": run, "started_utc": stamp, "file": filename})
    write_json(output / "run.json", {
        "started_utc": started,
        "finished_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "machine": machine(),
        "execution": execution,
    })
    summarize(output)


def summarize(output):
    values = {}
    for path in sorted(output.glob("*-run-*.jsonl")):
        mode, revision, _, run = path.stem.split("-")
        for row in map(json.loads, path.read_text(encoding="utf-8").splitlines()):
            values.setdefault(row["workload"], {}).setdefault((mode, revision), []).append(row)
    summary = []
    lines = [
        "# Stage 2 same-session measurements", "",
        "Five independent processes per revision and measurement mode; per-process call means are summarized below.",
        "Timing uses the normal system allocator. Allocation counts come from separate instrumented processes.",
        "Spread is minimum–maximum across five process means; no confidence interval or peak-memory claim.", "",
        "| Workload | Before median µs [min–max] | After median µs [min–max] | Speedup | Allocations before → after | Allocated bytes before → after |",
        "| --- | ---: | ---: | ---: | ---: | ---: |",
    ]
    for workload, measurements in sorted(values.items()):
        result = {"workload": workload}
        for revision in ["baseline", "candidate"]:
            timings = [row["elapsed_ns"] / row["iterations"] / 1000 for row in measurements[("timing", revision)]]
            allocations = measurements[("allocations", revision)]
            if len(timings) != RUNS or len(allocations) != RUNS:
                raise ValueError("incomplete process runs")
            result[revision] = {
                "median_us": statistics.median(timings),
                "min_us": min(timings), "max_us": max(timings),
                "median_absolute_deviation_us": statistics.median(abs(t - statistics.median(timings)) for t in timings),
                "process_means_us": timings,
            }
            for key in ["allocations", "reallocations", "deallocations", "bytes_allocated", "bytes_deallocated", "bytes_reallocated"]:
                observed = [row[key] / row["iterations"] for row in allocations]
                result[revision][key + "_per_call"] = statistics.median(observed)
                result[revision][key + "_range"] = [min(observed), max(observed)]
        before, after = result["baseline"], result["candidate"]
        result["median_speedup"] = before["median_us"] / after["median_us"]
        result["median_latency_change_percent"] = 100 * (after["median_us"] / before["median_us"] - 1)
        summary.append(result)
        lines.append(
            f"| {workload} | {before['median_us']:.2f} [{before['min_us']:.2f}–{before['max_us']:.2f}] "
            f"| {after['median_us']:.2f} [{after['min_us']:.2f}–{after['max_us']:.2f}] "
            f"| {result['median_speedup']:.2f}x | {before['allocations_per_call']:,.0f} → {after['allocations_per_call']:,.0f} "
            f"| {before['bytes_allocated_per_call']:,.0f} → {after['bytes_allocated_per_call']:,.0f} |"
        )
    write_json(output / "summary.json", summary)
    (output / "summary.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"Saved 50 workload summaries in {output}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    builder = commands.add_parser("build")
    builder.add_argument("--baseline", required=True)
    builder.add_argument("--candidate", default=".")
    builder.add_argument("--output", required=True)
    runner = commands.add_parser("run")
    runner.add_argument("--output", required=True)
    summarizer = commands.add_parser("summarize")
    summarizer.add_argument("--output", required=True)
    args = parser.parse_args()
    if args.action == "build":
        build(args)
    elif args.action == "run":
        measure(args)
    else:
        summarize(pathlib.Path(args.output).resolve())


if __name__ == "__main__":
    main()
