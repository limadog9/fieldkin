#!/usr/bin/env python3
"""Fresh-process Rust/Python comparisons; build the release parity example first.

python validation/benchmark_valentine.py --output target/valentine-performance.json
Uses the existing pinned reference and explicit reference-fix overlay. No accuracy
labels are used. Linux /proc VmHWM measures lifetime process peak RSS, including
setup and output verification/reporting, excluding child processes.
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


def side_order(baseline, round_number, native_only=False):
    sides = ("rust_before", "rust") if native_only else ("rust_before", "rust", "python") if baseline else ("rust", "python")
    offset = round_number % len(sides)
    return sides[offset:] + sides[:offset]


def native_identity(result):
    identity = result["output_sha256"]
    if (not isinstance(identity, str) or len(identity) != 64
            or any(character not in "0123456789abcdef" for character in identity)):
        raise ValueError("missing or invalid native exact-output fingerprint; rebuild the parity example")
    return identity


def markdown(report):
    baseline = report.get("settings", {}).get("baseline_revision") is not None
    sides = ("rust_before", "rust", "python") if baseline else ("rust", "python")
    lines = ["# Rust versus Python Valentine", "", "Matching medians are medians of fresh-process medians; ranges span those medians.",
             "RSS is median lifetime process VmHWM in MiB, including setup, inputs, matching and output verification/reporting; child processes are excluded.",
             "Rust samples before final serialization; Python after preceding checkpoint serialization. This is not incremental matcher or process-tree memory.",
             ("Native-only: Python is not executed. Native ratios require every run to succeed and exact before/after outputs."
              if report.get("settings", {}).get("native_only") else
              "Speedups require every run to succeed, repeat exactly within each runtime, and pass pair/score/ranking parity."),
             *(["Rust before/after also require identical ordered score/detail bits and result selections."] if baseline else []), "",
             ("| Case | Algorithm | Rust before ms [range] | Rust after ms [range] | Python ms [range] | Before/after/Python peak MiB | Python/after | Before/after | Status |" if baseline else
              "| Case | Algorithm | Rust ms [range] | Python ms [range] | Rust/Python peak MiB | Speedup | Status |"),
             "|---|---|---:|---:|---:|---:|---:|---:|---|" if baseline else "|---|---|---:|---:|---:|---:|---|"]
    for item in report["comparisons"]:
        cells = []
        for side in sides:
            result = item.get(side)
            cells.append(f"{result['matching_ms']['median']:.3f} [{result['matching_ms']['min']:.3f}, {result['matching_ms']['max']:.3f}]" if result else "unavailable")
        rss = "/".join(f"{item[side]['peak_rss_kib']['median']/1024:.1f}" if side in item else "n/a" for side in sides)
        speedup = f"{item['speedup']:.2f}x" if item.get("speedup") is not None else "withheld"
        improvement = f"{item['native_speedup']:.2f}x" if item.get("native_speedup") is not None else "withheld"
        lines.append(f"| {item['case']} | {item['algorithm']} | {' | '.join(cells)} | {rss} | {speedup} | "
                     + (f"{improvement} | " if baseline else "") + f"{item['status']} |")
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/examples/valentine_parity")
    parser.add_argument("--baseline-binary", type=Path, help="Unchanged Rust library built with the same parity example")
    parser.add_argument("--baseline-revision", help="Exact Git revision used to build --baseline-binary")
    parser.add_argument("--case", choices=(*SIZES, *REAL), action="append")
    parser.add_argument("--dataset", type=Path, action="append", help="Additional repository-relative schema JSON; supplied paths replace default cases")
    parser.add_argument("--algorithm", choices=ALGORITHMS, action="append")
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--upstream", action="store_true", help="Original unpatched reference (default: recorded fixes)")
    parser.add_argument("--native-only", action="store_true", help="Compare two Rust binaries without running Python")
    args = parser.parse_args()
    if min(args.rounds, args.repetitions, args.timeout) <= 0 or args.warmups < 0 or not math.isfinite(args.timeout):
        parser.error("rounds, repetitions, timeout must be positive; warmups nonnegative")
    if bool(args.baseline_binary) != bool(args.baseline_revision):
        parser.error("--baseline-binary and --baseline-revision must be supplied together")
    if args.native_only and not args.baseline_binary:
        parser.error("--native-only requires --baseline-binary and --baseline-revision")
    if args.baseline_revision:
        revision = command(["git", "rev-parse", "--verify", f"{args.baseline_revision}^{{commit}}"])
        if revision is None:
            parser.error("--baseline-revision must identify an existing commit in this repository")
        args.baseline_revision = revision
    binaries = {"rust": args.binary.resolve()}
    if args.baseline_binary:
        binaries = {"rust_before": args.baseline_binary.resolve(), **binaries}
    if sys.platform != "linux" or any(not path.is_file() or not os.access(path, os.X_OK) for path in binaries.values()):
        parser.error("requires Linux wait4 RSS accounting and executable release parity examples")
    destination = args.output.resolve()
    directory = destination.with_suffix("")
    try:
        directory.relative_to(ROOT)
    except ValueError:
        parser.error("output must be inside the repository (input hashes are verified there)")
    directory.mkdir(parents=True, exist_ok=True)
    paths = {}
    for name in args.case or (() if args.dataset else (*SIZES, *REAL)):
        path = directory / f"{name}.json" if name in SIZES else ROOT / f"eval_independent/{name}.json"
        if name in SIZES:
            path.write_text(json.dumps(synthetic(name, *SIZES[name]), separators=(",", ":")) + "\n")
        paths[name] = path.relative_to(ROOT).as_posix()
    for index, requested in enumerate(args.dataset or []):
        path = (ROOT / requested).resolve()
        try:
            relative = path.relative_to(ROOT).as_posix()
        except ValueError:
            parser.error(f"dataset must be inside the repository: {requested}")
        if not path.is_file() or relative in paths.values():
            parser.error(f"dataset must be an existing, distinct file: {requested}")
        paths[f"dataset_{index:03}_{path.stem}"] = relative
    cpu = read("/proc/cpuinfo") or ""
    environment = {"PYTHONHASHSEED": "0", "OMP_NUM_THREADS": "1", "OPENBLAS_NUM_THREADS": "1", "MKL_NUM_THREADS": "1"}
    corpora = [ROOT / ".upstream/nltk_data" / path for path in
               ("corpora/wordnet.zip", "corpora/stopwords.zip", "tokenizers/punkt_tab.zip")]
    metadata = {"nltk_archives": {path.relative_to(ROOT).as_posix():
                                hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None for path in corpora},
                "thread_environment": environment,"revision": command(["git", "rev-parse", "HEAD"]), "working_tree": command(["git", "status", "--short"]),
                "platform": platform.platform(), "python": sys.version, "rustc": command(["rustc", "-Vv"]),
                "cpu_model": next((line.split(":", 1)[1].strip() for line in cpu.splitlines() if line.startswith("model name")), None),
                "cpu_count": os.cpu_count(), "cpu_affinity": sorted(os.sched_getaffinity(0)),
                "cpu_max": read("/sys/fs/cgroup/cpu.max"), "memory_max": read("/sys/fs/cgroup/memory.max"),
                "input_sha256": {path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in paths.values()},
                "started_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "command": sys.argv,
                "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                "rust_binaries": {side: {"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                                         "revision": args.baseline_revision if side == "rust_before" else command(["git", "rev-parse", "HEAD"])}
                                  for side, path in binaries.items()},
                "baseline_build": ("Caller supplies the pinned baseline library built in a separate worktree/target directory, "
                                   "with this working tree's examples/valentine_parity.rs and examples/support/performance.rs copied unchanged. "
                                   "Binary-to-revision association is declared, not inferred from executable contents."
                                   if args.baseline_binary else None),
                "source_sha256": {path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in
                                  ("Cargo.lock", "examples/valentine_parity.rs", "examples/support/performance.rs", "scripts/run_valentine_parity.py", "scripts/compare_valentine_parity.py", "scripts/valentine_reference.py",
                                   "validation/benchmark_valentine.py", "validation/benchmark_performance.py",
                                   *(path.relative_to(ROOT).as_posix() for path in sorted((ROOT / "src").rglob("*.rs"))))}}
    report = {"metadata": metadata, "settings": {"rounds": args.rounds, "repetitions": args.repetitions, "warmups": args.warmups,
              "timeout_seconds": args.timeout, "reference": "upstream" if args.upstream else "fixed", "datasets": paths,
              "baseline_revision": args.baseline_revision,
              "matching_api": "get_matches on every side; loading, canonicalization, construction, result disposal, selection, hashing and serialization excluded",
              "rss_scope": "Linux lifetime process VmHWM including setup, inputs, matching, output verification and report construction; "
                           "Rust samples before final serialization, Python after preceding checkpoint serialization; excludes children (including CBC); "
                           "wait4 maximum is separate and includes inherited pre-exec watermark; neither is incremental matcher or aggregate process-tree memory",
              "native_only": args.native_only,
              "configuration": "existing parity constructors, defaults plus COMA instances and optional per-dataset jaccard_threshold; all nonempty samples; one worker"},
              "runs": [], "comparisons": []}
    for name, path in paths.items():
        for algorithm in args.algorithm or ALGORITHMS:
            originals, comparisons, native_comparisons = {}, [], []
            runs = []
            for round_number in range(args.rounds):
                documents = {}
                for side in side_order(args.baseline_binary is not None, round_number, args.native_only):
                    stem = directory / f"{name}-{algorithm}-{side}-{round_number}"
                    output = Path(str(stem) + ".json")
                    flags = ["--dataset", path, "--algorithm", algorithm, "--repetitions", str(args.repetitions), "--warmups", str(args.warmups)]
                    invocation = ([str(binaries[side]), str(output), "--direct", *flags] if side != "python" else
                                  [sys.executable, "scripts/run_valentine_parity.py", "--output", str(output), *flags,
                                   *([] if args.upstream else ["--reference-fixes"])])
                    run = {"case": name, "algorithm": algorithm, "side": side, "round": round_number + 1,
                           "command": invocation, "raw_report": output.relative_to(ROOT).as_posix()}
                    output.unlink(missing_ok=True)
                    started = time.perf_counter()
                    with Path(str(stem) + ".log").open("w") as log:
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
                            identity = (native_identity(result) if side != "python" else
                                        hashlib.sha256(json.dumps(result["matches"], sort_keys=True).encode()).hexdigest())
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
                if "rust_before" in documents and "rust" in documents:
                    before = documents["rust_before"]["datasets"][0]
                    after = documents["rust"]["datasets"][0]
                    native_comparisons.append({"round": round_number + 1,
                        "equivalent": before["input_sha256"] == after["input_sha256"]
                        and documents["rust_before"]["dtype_mapping"] == documents["rust"]["dtype_mapping"]
                        and native_identity(before["algorithms"][algorithm]) == native_identity(after["algorithms"][algorithm])})
                for side in binaries:
                    if side not in documents or "python" not in documents:
                        continue
                    rust, python = (documents[key]["datasets"][0] for key in (side, "python"))
                    context = (reference_provenance(documents["python"])["verified"] and
                               documents[side]["dtype_mapping"] == documents["python"]["dtype_mapping"])
                    comparison = compare_case(path, algorithm, rust, python, input_checks(path, rust, python), context)
                    comparison_path = directory / f"{name}-{algorithm}-{side}-comparison-{round_number}.json"
                    comparison_path.write_text(json.dumps(comparison, indent=2) + "\n")
                    comparisons.append({key: value for key, value in comparison.items() if not isinstance(value, (list, dict))} |
                                       {"side": side, "details": comparison_path.relative_to(ROOT).as_posix()})
            native_valid = not args.baseline_binary or (len(native_comparisons) == args.rounds and all(case["equivalent"] for case in native_comparisons))
            valid = ((args.native_only or (len(comparisons) == args.rounds * len(binaries) and all(case["equivalent"] for case in comparisons)))
                     and native_valid and all("error" not in run for run in runs))
            item = {"case": name, "algorithm": algorithm, "status": ("equivalent" if valid else "incomplete (execution failure)"
                               if any("error" in run for run in runs) else "non-equivalent"),
                    "parity": comparisons, "native_equivalence": native_comparisons, "speedup": None, "native_speedup": None}
            for side in (*binaries, "python"):
                successful = [run for run in runs if run["side"] == side and "error" not in run]
                if successful:
                    item[side] = {"matching_ms": summary([run["matching"]["median"] * 1000 for run in successful]),
                                  "peak_rss_kib": summary([run["peak_rss_kib"] for run in successful]),
                                  "process_wall_seconds": summary([run["process_wall_seconds"] for run in successful])}
            if valid:
                if not args.native_only:
                    item["speedup"] = item["python"]["matching_ms"]["median"] / item["rust"]["matching_ms"]["median"]
                if args.baseline_binary:
                    item["native_speedup"] = item["rust_before"]["matching_ms"]["median"] / item["rust"]["matching_ms"]["median"]
            report["comparisons"].append(item)
            destination.write_text(json.dumps(report, indent=2) + "\n")
            destination.with_suffix(".md").write_text(markdown(report))
            print(f"{name}/{algorithm}: {item['status']}; speedup {item['speedup']}", flush=True)
    return int(any(item["status"] != "equivalent" for item in report["comparisons"]))


if __name__ == "__main__":
    raise SystemExit(main())
