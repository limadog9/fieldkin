#!/usr/bin/env python3
"""Fresh-process Rust/Python comparisons; build the release parity example first.

python validation/benchmark_valentine.py --output target/valentine-performance.json
Uses the existing pinned reference and explicit reference-fix overlay. No accuracy
labels are used. Linux /proc VmHWM measures process peak RSS through matching, excluding children.
wait4 RSS is retained separately: it also includes inherited pre-exec memory.
"""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import signal
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from compare_valentine_parity import compare_case, input_checks, reference_provenance
from benchmark_performance import command, read

ALGORITHMS = ("coma", "coma_instances", "cupid", "distribution", "jaccard", "flooding")
SIZES = {"small": (4, 32, 16), "medium": (16, 1024, 128), "large": (32, 8192, 1024)}
REAL = ("01_penguins", "02_aircraft_flights")


def synthetic(name, columns, rows, distinct):
    """Half renamed columns; 75% distinct-value overlap, alternating int/text."""
    schemas = []
    for side in range(2):
        fields = []
        for column in range(columns):
            values = []
            for index in range(distinct):
                key = (column * 2 + (side if index >= distinct * 3 // 4 else 0)) * distinct + index
                values.append(str(key) if column % 2 == 0 else hashlib.sha256(str(key).encode()).hexdigest()[:16])
            fields.append({"name": f"{'attribute' if side and column % 2 else 'measure'}_{column:04}",
                           "data_type": "integer" if column % 2 == 0 else "text",
                           "samples": [values[row % distinct] for row in range(rows)]})
        schemas.append({"fields": fields})
    return {"name": f"synthetic_{name}", "source": schemas[0], "target": schemas[1]}


def summary(values):
    if not values or any(not math.isfinite(value) or value < 0 for value in values):
        raise ValueError("missing or invalid measurements")
    return {"median": statistics.median(values), "min": min(values), "max": max(values)}


def markdown(report):
    lines = ["# Rust versus Python Valentine", "", "Matching medians are medians of fresh-process medians; ranges span those medians.",
             "RSS is median process VmHWM in MiB through matching, including setup but excluding child processes.",
             "Speedups require every run to succeed, repeat exactly within each runtime, and pass pair/score/ranking parity.", "",
             "| Case | Algorithm | Rust ms [range] | Python ms [range] | Rust/Python peak MiB | Speedup | Status |",
             "|---|---|---:|---:|---:|---:|---|"]
    for item in report["comparisons"]:
        cells = []
        for side in ("rust", "python"):
            result = item.get(side)
            cells.append(f"{result['matching_ms']['median']:.3f} [{result['matching_ms']['min']:.3f}, {result['matching_ms']['max']:.3f}]" if result else "unavailable")
        rss = "/".join(f"{item[side]['peak_rss_kib']['median']/1024:.1f}" if side in item else "n/a" for side in ("rust", "python"))
        speedup = f"{item['speedup']:.2f}x" if item.get("speedup") is not None else "withheld"
        lines.append(f"| {item['case']} | {item['algorithm']} | {' | '.join(cells)} | {rss} | {speedup} | {item['status']} |")
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/examples/valentine_parity")
    parser.add_argument("--case", choices=(*SIZES, *REAL), action="append")
    parser.add_argument("--algorithm", choices=ALGORITHMS, action="append")
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--upstream", action="store_true", help="Original unpatched reference (default: recorded fixes)")
    args = parser.parse_args()
    if min(args.rounds, args.repetitions, args.timeout) <= 0 or args.warmups < 0 or not math.isfinite(args.timeout):
        parser.error("rounds, repetitions, timeout must be positive; warmups nonnegative")
    if sys.platform != "linux" or not args.binary.is_file():
        parser.error("requires Linux wait4 RSS accounting and a built release parity example")
    destination = args.output.resolve()
    directory = destination.with_suffix("")
    try:
        directory.relative_to(ROOT)
    except ValueError:
        parser.error("output must be inside the repository (input hashes are verified there)")
    directory.mkdir(parents=True, exist_ok=True)
    paths = {}
    for name in args.case or (*SIZES, *REAL):
        path = directory / f"{name}.json" if name in SIZES else ROOT / f"eval_independent/{name}.json"
        if name in SIZES:
            path.write_text(json.dumps(synthetic(name, *SIZES[name]), separators=(",", ":")) + "\n")
        paths[name] = path.relative_to(ROOT).as_posix()
    cpu = read("/proc/cpuinfo") or ""
    environment = {"PYTHONHASHSEED": "0", "OMP_NUM_THREADS": "1", "OPENBLAS_NUM_THREADS": "1", "MKL_NUM_THREADS": "1"}
    corpora = [ROOT / ".upstream/nltk_data" / path for path in
               ("corpora/wordnet.zip", "corpora/stopwords.zip", "tokenizers/punkt_tab.zip")]
    metadata = {"nltk_archives": {path.relative_to(ROOT).as_posix():
                                hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None for path in corpora},
                "thread_environment": environment,"revision": command(["git", "rev-parse", "HEAD"]), "working_tree": command(["git", "status", "--short"]),
                "platform": platform.platform(), "python": sys.version, "rustc": command(["rustc", "-Vv"]),
                "cpu_model": next((line.split(":", 1)[1].strip() for line in cpu.splitlines() if line.startswith("model name")), None),
                "cpu_count": os.cpu_count(), "cpu_max": read("/sys/fs/cgroup/cpu.max"), "memory_max": read("/sys/fs/cgroup/memory.max"),
                "input_sha256": {path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in paths.values()},
                "started_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "command": sys.argv,
                "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                "source_sha256": {path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in
                                  ("Cargo.lock", "examples/valentine_parity.rs", "scripts/run_valentine_parity.py", "scripts/compare_valentine_parity.py", "scripts/valentine_reference.py",
                                   "validation/benchmark_valentine.py", "validation/benchmark_performance.py",
                                   *(path.relative_to(ROOT).as_posix() for path in sorted((ROOT / "src").rglob("*.rs"))))}}
    report = {"metadata": metadata, "settings": {"rounds": args.rounds, "repetitions": args.repetitions, "warmups": args.warmups,
              "timeout_seconds": args.timeout, "reference": "upstream" if args.upstream else "fixed", "datasets": paths,
              "matching_api": "get_matches on both sides; loading, canonicalization, construction and serialization excluded",
              "rss_scope": "Linux VmHWM through matching, excluding children; wait4 maximum is separate and includes inherited pre-exec watermark; neither is aggregate process-tree memory",
              "configuration": "existing parity constructors, defaults plus COMA instances; all nonempty samples; one worker"},
              "runs": [], "comparisons": []}
    for name, path in paths.items():
        for algorithm in args.algorithm or ALGORITHMS:
            originals, comparisons = {}, []
            runs = []
            for round_number in range(args.rounds):
                documents = {}
                for side in (("rust", "python") if round_number % 2 == 0 else ("python", "rust")):
                    stem = directory / f"{name}-{algorithm}-{side}-{round_number}"
                    output = stem.with_suffix(".json")
                    flags = ["--dataset", path, "--algorithm", algorithm, "--repetitions", str(args.repetitions), "--warmups", str(args.warmups)]
                    invocation = ([str(args.binary.resolve()), str(output), "--direct", *flags] if side == "rust" else
                                  [sys.executable, "scripts/run_valentine_parity.py", "--output", str(output), *flags,
                                   *([] if args.upstream else ["--reference-fixes"])])
                    run = {"case": name, "algorithm": algorithm, "side": side, "round": round_number + 1,
                           "command": invocation, "raw_report": output.relative_to(ROOT).as_posix()}
                    output.unlink(missing_ok=True)
                    started = time.perf_counter()
                    with stem.with_suffix(".log").open("w") as log:
                        process = subprocess.Popen(invocation,
                                                   cwd=ROOT, stdout=log, stderr=log, start_new_session=True, env=os.environ | environment)
                        try:
                            while True:
                                pid, status, usage = os.wait4(process.pid, os.WNOHANG)
                                if pid:
                                    process.returncode = os.waitstatus_to_exitcode(status)
                                    run.update(returncode=process.returncode, wait4_peak_rss_kib=usage.ru_maxrss)
                                    break
                                if time.perf_counter() - started >= args.timeout:
                                    raise subprocess.TimeoutExpired(invocation, args.timeout)
                                time.sleep(0.01)
                            if process.returncode:
                                raise ValueError(f"exit {process.returncode}; see {stem.name}.log")
                            document = json.loads(output.read_text())
                            dataset = document["datasets"][0]
                            result = dataset["algorithms"][algorithm]
                            times = result["matching_seconds"]
                            if len(times) != args.repetitions or len(result["warmup_seconds"]) != args.warmups or result["error"]:
                                raise ValueError("failed or incomplete repeated matching")
                            identity = hashlib.sha256(json.dumps(result["matches"], sort_keys=True).encode()).hexdigest()
                            if originals.setdefault(side, identity) != identity:
                                raise ValueError("nondeterministic output across fresh processes")
                            run.update(matching_seconds=times, warmup_seconds=result["warmup_seconds"], matching=summary(times),
                                       peak_rss_kib=document["peak_rss_kib"], output_sha256=identity,
                                       input_loading_seconds=dataset["input_loading_seconds"],
                                       preparation_seconds=result.get("preparation_seconds"), setup_seconds=document.get("setup_seconds"))
                            documents[side] = document
                            if side == "python":
                                report["metadata"]["reference"] = document["reference"]
                                for key in ("dependency_versions", "runtime_dependencies", "distribution_metadata"):
                                    report["metadata"][key] = document[key]
                                report["metadata"].setdefault("configurations", {}).update(document["configurations"])
                        except (ValueError, KeyError, OSError, subprocess.TimeoutExpired) as error:
                            run.update(error=str(error), error_type=type(error).__name__)
                            if process.poll() is None:
                                os.killpg(process.pid, signal.SIGKILL)
                                process.wait()
                            run["returncode"] = process.returncode
                    run["process_wall_seconds"] = time.perf_counter() - started
                    runs.append(run)
                    report["runs"].append(run)
                    destination.write_text(json.dumps(report, indent=2) + "\n")
                if len(documents) == 2:
                    rust, python = (documents[side]["datasets"][0] for side in ("rust", "python"))
                    context = (reference_provenance(documents["python"])["verified"] and
                               documents["rust"]["dtype_mapping"] == documents["python"]["dtype_mapping"])
                    comparison = compare_case(path, algorithm, rust, python, input_checks(path, rust, python), context)
                    comparison_path = directory / f"{name}-{algorithm}-comparison-{round_number}.json"
                    comparison_path.write_text(json.dumps(comparison, indent=2) + "\n")
                    comparisons.append({key: value for key, value in comparison.items() if not isinstance(value, (list, dict))} |
                                       {"details": comparison_path.relative_to(ROOT).as_posix()})
            valid = len(comparisons) == args.rounds and all(case["equivalent"] for case in comparisons) and all("error" not in run for run in runs)
            item = {"case": name, "algorithm": algorithm, "status": ("equivalent" if valid else "incomplete (execution failure)"
                               if any("error" in run for run in runs) else "non-equivalent"),
                    "parity": comparisons, "speedup": None}
            for side in ("rust", "python"):
                successful = [run for run in runs if run["side"] == side and "error" not in run]
                if successful:
                    item[side] = {"matching_ms": summary([run["matching"]["median"] * 1000 for run in successful]),
                                  "peak_rss_kib": summary([run["peak_rss_kib"] for run in successful]),
                                  "process_wall_seconds": summary([run["process_wall_seconds"] for run in successful])}
            if valid:
                item["speedup"] = item["python"]["matching_ms"]["median"] / item["rust"]["matching_ms"]["median"]
            report["comparisons"].append(item)
            destination.write_text(json.dumps(report, indent=2) + "\n")
            destination.with_suffix(".md").write_text(markdown(report))
            print(f"{name}/{algorithm}: {item['status']}; speedup {item['speedup']}", flush=True)
    return int(any(item["speedup"] is None for item in report["comparisons"]))


if __name__ == "__main__":
    raise SystemExit(main())
