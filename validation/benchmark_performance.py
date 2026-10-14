#!/usr/bin/env python3
"""Run fresh release processes, retain raw measurements, and check exact outputs.

Build binaries separately. Timings are observations, never CI pass/fail thresholds.
"""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent


def cases():
    suite = {}
    base = dict(columns=8, rows=128, distinct=32, tables=2, overlap=50, schema_overlap=100, values="mixed")
    profiles = [
        ("base", {}), ("width", dict(columns=32)),
        ("rows", dict(rows=8192)), ("distinct", dict(columns=4, rows=256, distinct=256)),
        ("tables", dict(tables=4)), ("overlap0", dict(overlap=0)),
        ("schema0", dict(schema_overlap=0)), ("schema50", dict(schema_overlap=50)),
        ("overlap100", dict(overlap=100)), ("sampled", dict(rows=8192, sample_size=128)),
    ]
    for algorithm in ("coma", "cupid", "distribution", "jaccard", "flooding"):
        for name, changes in profiles:
            suite[f"{algorithm}-{name}"] = dict(base, algorithm=algorithm, **changes)
    for width in (64, 128):
        suite[f"flooding-width{width}"] = dict(base, algorithm="flooding", columns=width, rows=0)
    for width in (256, 512):
        suite[f"distribution-width{width}"] = dict(base, algorithm="distribution", columns=width,
                                                   rows=32, distinct=1, overlap=100)
    suite["jaccard-distinct512"] = dict(base, algorithm="jaccard", rows=512, distinct=512, overlap=100)
    suite["jaccard-codes32"] = dict(base, algorithm="jaccard", values="codes", overlap=100)
    suite["jaccard-codes512"] = dict(base, algorithm="jaccard", values="codes", rows=512, distinct=512, overlap=100)
    for width in (8, 16, 32, 64):
        suite[f"distribution-interleaved{width}"] = dict(base, algorithm="distribution", values="interleaved",
                                                        columns=width, rows=128, distinct=128, overlap=100)
    for width in (8, 12, 16):
        suite[f"distribution-ring{width}"] = dict(base, algorithm="distribution", values="ring",
                                                 columns=width, rows=128, distinct=128, overlap=100)
    for overlap in (0, 50, 100):
        suite[f"jaccard-single2048-overlap{overlap}"] = dict(base, algorithm="jaccard", values="hashes", columns=1,
                                                            rows=2048, distinct=2048, overlap=overlap)
    for algorithm in ("csv", "json"):
        for rows in (10000, 100000):
            suite[f"{algorithm}-rows{rows}"] = dict(base, algorithm=algorithm, rows=rows, distinct=256)
    for algorithm in ("top0", "top10", "filter", "per-source"):
        suite[f"results-{algorithm}"] = dict(base, algorithm=algorithm, columns=64, rows=50000)
    for algorithm in ("metrics", "curve"):
        suite[f"results-{algorithm}"] = dict(base, algorithm=algorithm, columns=64)
    for algorithm in ("hungarian", "hungarian-aware"):
        for columns, target_columns in ((1, 256), (256, 1), (8, 128), (128, 8)):
            suite[f"{algorithm}-{columns}x{target_columns}"] = dict(
                base, algorithm=algorithm, columns=columns, target_columns=target_columns)
        suite[f"{algorithm}-tied512x1"] = dict(
            base, algorithm=algorithm, columns=512, target_columns=1, result_scores="tied")
        for columns, target_columns in ((512, 1), (1, 512), (1024, 8), (8, 1024),
                                        (4096, 16), (16, 4096), (10000, 1), (1, 10000),
                                        (64, 64), (128, 128)):
            suite[f"{algorithm}-varied{columns}x{target_columns}"] = dict(
                base, algorithm=algorithm, columns=columns, target_columns=target_columns)
        for scores in ("tied", "sparse"):
            for columns, target_columns in ((1024, 8), (8, 1024), (64, 64)):
                suite[f"{algorithm}-{scores}{columns}x{target_columns}"] = dict(
                    base, algorithm=algorithm, columns=columns, target_columns=target_columns,
                    result_scores=scores)
        for columns, target_columns in ((1024, 8), (8, 1024), (64, 64)):
            suite[f"{algorithm}-guarded{columns}x{target_columns}"] = dict(
                base, algorithm=f"{algorithm}-guarded", columns=columns, target_columns=target_columns)
    for workers in (1, 4):
        suite[f"jaccard-empty-workers{workers}"] = dict(
            base, algorithm="jaccard", columns=512, target_columns=768, rows=0, workers=workers)
        suite[f"jaccard-exact-disjoint-workers{workers}"] = dict(
            base, algorithm="jaccard-exact", columns=128, target_columns=192,
            rows=8, distinct=8, overlap=0, workers=workers)
        suite[f"jaccard-dense-workers{workers}"] = dict(
            base, algorithm="jaccard", columns=64, target_columns=96, rows=2, distinct=2,
            values="codes", workers=workers)
        suite[f"jaccard-batch-workers{workers}"] = dict(
            base, algorithm="jaccard", columns=64, rows=0, tables=4, workers=workers)
    return suite


def read(path):
    try:
        return Path(path).read_text().strip()
    except OSError:
        return None


def command(args):
    try:
        return subprocess.check_output(args, cwd=ROOT, text=True, stderr=subprocess.DEVNULL).strip()
    except (OSError, subprocess.CalledProcessError):
        return None


def rectangular_algorithm(algorithm):
    """Explicit opt-in mapping; other operations must not silently pass through."""
    if algorithm not in ("hungarian", "hungarian-aware", "hungarian-guarded", "hungarian-aware-guarded"):
        raise ValueError("rectangular comparison requires a Hungarian assignment case")
    return algorithm.replace("hungarian", "rectangular", 1)


def assignment_identity(report):
    """Compare exact input/objective, rather than equating optimal tied pairs."""
    validation = report["assignment_validation"]
    if not isinstance(validation, dict):
        raise ValueError("missing assignment validation")
    for key in ("input_sha256", "selected_sha256"):
        value = validation[key]
        if not isinstance(value, str) or len(value) != 64 or any(c not in "0123456789abcdef" for c in value):
            raise ValueError(f"invalid assignment {key}")
    objective = validation["objective"]
    cardinality = validation["eligible_pair_count"]
    units = validation["score_units"]
    if type(units) is not int or units < 0:
        raise ValueError("invalid exact-grid objective sum")
    if objective == "eligible-cardinality-then-score":
        if type(cardinality) is not int or cardinality < 0 or cardinality != report["result_count"]:
            raise ValueError("invalid eligible assignment cardinality")
    elif objective == "score-before-filter":
        if cardinality is not None:
            raise ValueError("maximum-score objective must be checked before threshold filtering")
    else:
        raise ValueError("unknown assignment objective")
    return validation["input_sha256"], objective, cardinality, units


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    executable = "benchmark_performance.exe" if os.name == "nt" else "benchmark_performance"
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/examples" / executable)
    parser.add_argument("--baseline-binary", type=Path)
    parser.add_argument("--compare-rectangular", action="store_true",
                        help="explicitly compare legacy before/new rectangular after: exact objective, allowing optimal tied pairs")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--repetitions", type=int, default=5)
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=180, help="seconds per process")
    parser.add_argument("--memory-limit-mib", type=int, help="Unix child virtual-address-space limit, not peak RSS")
    parser.add_argument("--case", action="append", choices=list(cases()), help="repeat to select cases")
    parser.add_argument("--list", action="store_true")
    args = parser.parse_args()
    if args.list:
        print("\n".join(cases()))
        return 0
    if args.compare_rectangular:
        if args.baseline_binary is None or not args.case:
            parser.error("--compare-rectangular requires --baseline-binary and explicit --case selections")
        try:
            for name in args.case:
                rectangular_algorithm(cases()[name]["algorithm"])
        except ValueError as error:
            parser.error(str(error))
    if (args.output is None or min(args.repetitions, args.rounds, args.timeout) <= 0
            or not math.isfinite(args.timeout)):
        parser.error("--output is required; repetitions, rounds, and timeout must be positive")
    child_limits = None
    if args.memory_limit_mib is not None:
        if os.name != "posix" or args.memory_limit_mib <= 0:
            parser.error("--memory-limit-mib requires Unix and a positive limit")
        import resource
        if not hasattr(resource, "RLIMIT_AS"):
            parser.error("--memory-limit-mib requires RLIMIT_AS support")

        def child_limits():
            limit = args.memory_limit_mib * 1024 * 1024
            resource.setrlimit(resource.RLIMIT_AS, (limit, limit))
    binaries = {"after": args.binary.resolve()}
    if args.baseline_binary:
        binaries = {"before": args.baseline_binary.resolve(), **binaries}
    for path in binaries.values():
        if not path.is_file() or not os.access(path, os.X_OK):
            parser.error(f"binary is not executable: {path}")
    cpu = read("/proc/cpuinfo") or ""
    metadata = dict(platform=platform.platform(), python=platform.python_version(),
                    rustc=command(["rustc", "--version"]), revision=command(["git", "rev-parse", "HEAD"]),
                    working_tree=command(["git", "status", "--short"]), cpu_count=os.cpu_count(),
                    cpu_model=next((line.split(":", 1)[1].strip() for line in cpu.splitlines()
                                    if line.startswith("model name")), None),
                    cpu_max=read("/sys/fs/cgroup/cpu.max"), memory_max=read("/sys/fs/cgroup/memory.max"),
                    started_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), binaries={})
    for version, path in binaries.items():
        with path.open("rb") as binary:
            metadata["binaries"][version] = dict(path=str(path), sha256=hashlib.file_digest(binary, "sha256").hexdigest())
    if len(binaries) == 2 and len({entry["sha256"] for entry in metadata["binaries"].values()}) == 1:
        parser.error("comparison binaries are identical; build revisions in separate target directories")
    output = dict(metadata=metadata, settings=vars(args) | {"binary": str(args.binary),
                  "baseline_binary": str(args.baseline_binary) if args.baseline_binary else None,
                  "output": str(args.output)}, runs=[], failures=[])
    args.output.parent.mkdir(parents=True, exist_ok=True)
    reference = {}
    selected_hashes = {}
    for name, parameters in cases().items():
        if args.case and name not in args.case:
            continue
        expected = dict(parameters, repetitions=args.repetitions)
        expected.setdefault("sample_size", None)
        expected.setdefault("target_columns", None)
        expected.setdefault("workers", 1)
        expected.setdefault("result_scores", "mixed")
        for round_number in range(args.rounds):
            versions = list(binaries) if round_number % 2 == 0 else list(reversed(binaries))
            for version in versions:
                actual = dict(expected)
                if args.compare_rectangular and version == "after":
                    actual["algorithm"] = rectangular_algorithm(actual["algorithm"])
                flags = [item for key, value in actual.items() if value is not None
                         for item in ("--" + key.replace("_", "-"), str(value))]
                run = dict(case=name, round=round_number + 1, version=version)
                try:
                    process = subprocess.run([str(binaries[version]), *flags], capture_output=True,
                                             text=True, timeout=args.timeout, check=False, preexec_fn=child_limits)
                    run.update(stdout=process.stdout, stderr=process.stderr, returncode=process.returncode)
                    if process.returncode:
                        raise ValueError(f"process exited {process.returncode}: {process.stderr.strip()}")
                    report = json.loads(process.stdout)
                    run["report"] = report
                    if (not math.isfinite(report["median_ms"]) or report["median_ms"] < 0
                            or len(report["milliseconds"]) != args.repetitions):
                        raise ValueError("invalid or incomplete timing measurements")
                    if report["parameters"] != actual:
                        raise ValueError(f"parameters differ: expected {actual}, got {report['parameters']}")
                    if args.compare_rectangular:
                        identity = assignment_identity(report)
                        hashes = selected_hashes.setdefault(name, {})
                        selected = report["assignment_validation"]["selected_sha256"]
                        if hashes.setdefault(version, selected) != selected:
                            raise ValueError("selected pairs/bits differ across repeated runs of the same solver")
                        run["comparison"] = "exact objective; tied selected pairs may differ"
                    else:
                        # Existing exact-output gate remains the default.
                        identity = (report["output_sha256"], report["result_count"], report["parameters"])
                    if reference.setdefault(name, identity) != identity:
                        raise ValueError("assignment input/objective differs across versions/rounds" if args.compare_rectangular
                                         else "output fingerprint or result count differs across versions/rounds")
                except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
                    run["error"] = str(error)
                    if isinstance(error, subprocess.TimeoutExpired):
                        run.update(stdout=(error.stdout or b"").decode(errors="replace"),
                                   stderr=(error.stderr or b"").decode(errors="replace"))
                    diagnostic = f"{name} {version} round {round_number + 1}: {error}"
                    output["failures"].append(diagnostic)
                    print(diagnostic, file=sys.stderr, flush=True)
                output["runs"].append(run)
                args.output.write_text(json.dumps(output, indent=2) + "\n")
        summary = []
        for version in binaries:
            reports = [run["report"] for run in output["runs"]
                       if run["case"] == name and run["version"] == version and "error" not in run]
            if reports:
                median = statistics.median(report["median_ms"] for report in reports)
                rss = [report["peak_rss_kib"] for report in reports if report["peak_rss_kib"] is not None]
                summary.append(f"{version}: {median:.3f} ms, RSS {statistics.median(rss) if rss else 'n/a'} KiB")
        if args.compare_rectangular:
            hashes = selected_hashes.get(name, {})
            case_runs = [run for run in output["runs"] if run["case"] == name]
            if any("error" in run for run in case_runs):
                summary.append("comparison incomplete/failed; no speedup established")
            elif len(hashes) == 2:
                summary.append("identical ordered pairs/bits" if len(set(hashes.values())) == 1
                               else "different tied pairs; identical exact objective")
        print(f"{name}: {'; '.join(summary) or 'failed'}", flush=True)
    return int(bool(output["failures"]))


if __name__ == "__main__":
    sys.exit(main())
