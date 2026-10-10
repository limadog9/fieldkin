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
    for algorithm in ("csv", "json"):
        for rows in (10000, 100000):
            suite[f"{algorithm}-rows{rows}"] = dict(base, algorithm=algorithm, rows=rows, distinct=256)
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    executable = "benchmark_performance.exe" if os.name == "nt" else "benchmark_performance"
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/examples" / executable)
    parser.add_argument("--baseline-binary", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--repetitions", type=int, default=5)
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=180, help="seconds per process")
    parser.add_argument("--case", action="append", choices=list(cases()), help="repeat to select cases")
    parser.add_argument("--list", action="store_true")
    args = parser.parse_args()
    if args.list:
        print("\n".join(cases()))
        return 0
    if (args.output is None or min(args.repetitions, args.rounds, args.timeout) <= 0
            or not math.isfinite(args.timeout)):
        parser.error("--output is required; repetitions, rounds, and timeout must be positive")
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
    for name, parameters in cases().items():
        if args.case and name not in args.case:
            continue
        expected = dict(parameters, repetitions=args.repetitions)
        expected.setdefault("sample_size", None)
        for round_number in range(args.rounds):
            versions = list(binaries) if round_number % 2 == 0 else list(reversed(binaries))
            for version in versions:
                flags = [item for key, value in expected.items() if value is not None
                         for item in ("--" + key.replace("_", "-"), str(value))]
                run = dict(case=name, round=round_number + 1, version=version)
                try:
                    process = subprocess.run([str(binaries[version]), *flags], capture_output=True,
                                             text=True, timeout=args.timeout, check=False)
                    run.update(stdout=process.stdout, stderr=process.stderr, returncode=process.returncode)
                    if process.returncode:
                        raise ValueError(f"process exited {process.returncode}: {process.stderr.strip()}")
                    report = json.loads(process.stdout)
                    run["report"] = report
                    if (not math.isfinite(report["median_ms"]) or report["median_ms"] < 0
                            or len(report["milliseconds"]) != args.repetitions):
                        raise ValueError("invalid or incomplete timing measurements")
                    if report["parameters"] != expected:
                        raise ValueError(f"parameters differ: expected {expected}, got {report['parameters']}")
                    identity = (report["output_sha256"], report["result_count"], report["parameters"])
                    if reference.setdefault(name, identity) != identity:
                        raise ValueError("output fingerprint or result count differs across versions/rounds")
                except (OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
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
        print(f"{name}: {'; '.join(summary) or 'failed'}", flush=True)
    return int(bool(output["failures"]))


if __name__ == "__main__":
    sys.exit(main())
