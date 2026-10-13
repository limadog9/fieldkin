#!/usr/bin/env python3
"""Reproduce the rejected frozen candidate without changing repository runtime."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
WORKLOADS = (
    "exposed597", "aligned_small", "aligned_medium", "aligned_large",
    "ambiguous_small", "ambiguous_medium", "ambiguous_large",
)


def command(args, cwd=None):
    return subprocess.check_output(args, cwd=cwd, text=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "target/semantic_performance.json")
    parser.add_argument("--verify-only", action="store_true", help="check archived outputs without timing runs")
    args = parser.parse_args()
    measured = json.loads((HERE / "performance_results.json").read_text())
    revision = measured["git_revision"]
    freeze = json.loads((HERE / "candidate_freeze.json").read_text())
    if revision != freeze["baseline_revision"]:
        raise SystemExit("Performance base revision differs from the frozen candidate")
    exists = subprocess.run(["git", "cat-file", "-e", f"{revision}^{{commit}}"], cwd=ROOT, capture_output=True)
    if exists.returncode:
        raise SystemExit(f"Base revision is missing; fetch it with: git fetch origin {revision}")
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "measured_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "method": measured["method"],
        "git_revision": revision,
        "candidate_patch_sha256": hashlib.sha256((HERE / "candidate.patch").read_bytes()).hexdigest(),
        "harness_sha256": hashlib.sha256((HERE / "benchmark_corroboration.rs").read_bytes()).hexdigest(),
        "environment": {"platform": platform.platform(), "rustc": command(["rustc", "-Vv"])},
        "workloads": {}, "failures": [], "verification_only": args.verify_only,
    }
    for name, path in [("cpu_quota", "/sys/fs/cgroup/cpu.max"), ("memory_limit", "/sys/fs/cgroup/memory.max")]:
        if Path(path).exists():
            report["environment"][name] = Path(path).read_text().strip()
    if platform.system() == "Linux":
        report["environment"]["cpu"] = command(["lscpu"])
    with tempfile.TemporaryDirectory(prefix="fieldkin-corroboration-") as temporary:
        temporary = Path(temporary)
        checkout = temporary / "repository"
        checkout.mkdir()
        archive = temporary / "source.tar"
        subprocess.run(["git", "archive", "--format=tar", f"--output={archive}", revision], cwd=ROOT, check=True)
        subprocess.run(["tar", "xf", str(archive), "-C", str(checkout)], check=True)
        subprocess.run(["git", "apply", "--check", str(HERE / "candidate.patch")], cwd=checkout, check=True)
        subprocess.run(["git", "apply", str(HERE / "candidate.patch")], cwd=checkout, check=True)
        for filename, expected in freeze["runtime_files"].items():
            if hashlib.sha256((checkout / filename).read_bytes()).hexdigest() != expected:
                raise SystemExit(f"Patched runtime differs from the frozen candidate: {filename}")
        (checkout / "examples/semantic_performance.rs").write_text((HERE / "benchmark_corroboration.rs").read_text())
        target = ROOT / "target/semantic-performance"
        subprocess.run([
            "cargo", "build", "--release", "--locked", "--jobs", "1",
            "--manifest-path", str(checkout / "Cargo.toml"),
            "--target-dir", str(target), "--example", "semantic_performance",
        ], check=True)
        suffix = ".exe" if os.name == "nt" else ""
        binary = target / f"release/examples/semantic_performance{suffix}"
        report["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
        for workload in WORKLOADS:
            verify = json.loads(command([str(binary), "verify", workload]))
            if verify != measured["workloads"][workload]["verification"]:
                raise ValueError(f"{workload}: outputs differ from the frozen experiment")
            result = report["workloads"][workload] = {"verification": verify, "runs": {"original": [], "proposed": []}}
            if args.verify_only:
                print(f"{workload}: archived outputs and candidate bits verified", flush=True)
                output.write_text(json.dumps(report, indent=2) + "\n")
                continue
            for process in range(3):
                modes = ("original", "proposed") if process % 2 == 0 else ("proposed", "original")
                for mode in modes:
                    try:
                        completed = subprocess.run([str(binary), mode, workload], capture_output=True, text=True, timeout=120, check=True)
                        run = json.loads(completed.stdout)
                        if run["result_digest"] != verify[f"{mode}_digest"]:
                            raise ValueError("matching output changed between verification and measurement")
                        result["runs"][mode].append(run)
                        print(f"{workload} {mode} process {process}: {statistics.median(run['times_ms']):.3f} ms", flush=True)
                    except (subprocess.SubprocessError, ValueError) as error:
                        report["failures"].append({"workload": workload, "mode": mode, "process": process, "error": str(error)})
                    output.write_text(json.dumps(report, indent=2) + "\n")
            result["summary"] = {}
            for mode, runs in result["runs"].items():
                if runs:
                    medians = [statistics.median(run["times_ms"]) for run in runs]
                    rss = [run["peak_rss_kib"] for run in runs if run["peak_rss_kib"] is not None]
                    result["summary"][mode] = {"median_ms": statistics.median(medians), "process_median_min_ms": min(medians), "process_median_max_ms": max(medians), "peak_rss_median_kib": statistics.median(rss) if rss else None}
            output.write_text(json.dumps(report, indent=2) + "\n")
    if report["failures"]:
        raise SystemExit(f"Benchmark failures recorded in {output}")
    print(f"Results: {output}")


if __name__ == "__main__":
    main()
