#!/usr/bin/env python3
"""One frozen, bounded 128/512/1000-field scale characterization, not a scale guarantee."""

import argparse
import datetime
import hashlib
import json
import math
import pathlib
import statistics
import sys

# Reuse the existing scoped hashing, Cargo configuration and compiler helpers.
# Their exact source file is included in this experiment's inventory.
import assignment


ROOT = pathlib.Path(__file__).resolve().parent.parent
PROTOCOL = "fieldkin-scale-v1"
RUNS = 5
KEYS = ["family", "size", "one_to_one", "iterations", "elapsed_ns", "selected", "unmatched",
        "pairs", "signal_evaluations", "diagnostic_solves", "diagnostic_work"]
SIZES = [128, 512, 1000]
FAMILIES = ["diagonal", "partial", "builtin"]


def inventory():
    paths = [path for path in (ROOT / "src").rglob("*") if path.is_file()]
    paths += [ROOT / name for name in ["Cargo.toml", "Cargo.lock", "README.md", "evaluation/Cargo.toml",
                                      "examples/scale_cost.rs", "performance/scale.py", "performance/assignment.py"]]
    for directory in [ROOT, ROOT / "evaluation"]:
        paths += [directory / name for name in ["build.rs", "rust-toolchain", "rust-toolchain.toml"]
                  if (directory / name).exists()]
    return dict(sorted((path.relative_to(ROOT).as_posix(), assignment.digest(path)) for path in paths))


def validate(raw, small=False):
    rows = [json.loads(line) for line in raw.splitlines()]
    expected = [(size, family, policy) for size in ([128] if small else SIZES)
                for family in FAMILIES for policy in [False, True]]
    actual = []
    for row in rows:
        if set(row) != set(KEYS):
            raise ValueError("unexpected scale output fields")
        size, family, policy = row["size"], row["family"], row["one_to_one"]
        actual.append((size, family, policy))
        if type(policy) is not bool or type(size) is not int or size not in SIZES or family not in FAMILIES:
            raise ValueError("invalid scale workload identity")
        selected = size - (size // 8 if family == "partial" else 0)
        expected_counts = {"iterations": 3 if size == 128 else 1, "selected": selected,
                           "unmatched": size - selected, "pairs": size * size,
                           "signal_evaluations": size * size * (3 if family == "builtin" else 1),
                           "diagnostic_solves": 0, "diagnostic_work": 0}
        if any(type(row[key]) is not int or row[key] != count for key, count in expected_counts.items()):
            raise ValueError("scale workload changed decisions, pair counts or diagnostic work")
        if type(row["elapsed_ns"]) is not int or row["elapsed_ns"] < 0 or not math.isfinite(row["elapsed_ns"]):
            raise ValueError("invalid scale duration")
    if actual != expected:
        raise ValueError("missing, duplicate or reordered scale workloads")
    return rows


def verify(meta):
    if meta["protocol"] != PROTOCOL or meta["runs"] != RUNS or meta["sizes"] != SIZES or meta["families"] != FAMILIES:
        raise ValueError("scale protocol changed")
    if meta["source_sha256"] != inventory() or meta["cargo_config_sha256"] != assignment.cargo_configs(ROOT):
        raise ValueError("scale source inventory or Cargo configuration changed")
    if meta["compiler"] != assignment.compiler() or meta["build_environment"] != assignment.environment():
        raise ValueError("scale compiler or build environment changed")
    if meta["binary_sha256"] != assignment.digest(pathlib.Path(meta["binary_path"])):
        raise ValueError("scale executable changed")


def build(out):
    if out.exists() and any(out.iterdir()):
        raise ValueError("use a fresh empty scale output directory")
    out.mkdir(parents=True, exist_ok=True)
    before = inventory()
    target = ROOT / "target" / "scale-cost" / hashlib.sha256(str(out).encode()).hexdigest()[:16]
    target.mkdir(parents=True, exist_ok=False)
    meta = {"protocol": PROTOCOL, "commit": assignment.capture(["git", "rev-parse", "HEAD"], ROOT),
            "git_status": assignment.capture(["git", "status", "--short"], ROOT),
            "source_sha256": before, "cargo_config_sha256": assignment.cargo_configs(ROOT),
            "compiler": assignment.compiler(), "build_environment": assignment.environment(),
            "runs": RUNS, "sizes": SIZES, "families": FAMILIES, "machine": assignment.machine(),
            "warmup_calls": 1, "iterations": {"128": 3, "512": 1, "1000": 1},
            "timing_scope": "matching and report destruction; excludes fixture/engine creation and warmup report validation",
            "config": {"min_score": 0.70, "ambiguity_margin": 0.08, "max_candidates": 1,
                       "abstain_on_ambiguity": True, "global_diagnostics_max_solves": 0,
                       "limits": {"max_fields": "size", "max_pairs": "size*size",
                                  "max_signal_evaluations": "size*size*(3 for builtin, else 1)",
                                  "max_explanation_bytes": 536870912},
                       "builtin": "default name/type/sample matchers; 16 distinct Integer samples per field, shared only with its diagonal target",
                       "custom": "one fixed matcher:0.9 only on the diagonal; partial removes every eighth diagonal edge; zero elsewhere"},
            "built_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "cli": [sys.executable, str(pathlib.Path(__file__).resolve()), *sys.argv[1:]]}
    cmd = ["cargo", "+" + assignment.TOOLCHAIN, "build", "--release", "--locked", "-p", "fieldkin",
           "--example", "scale_cost", "--target-dir", str(target), "--message-format=json"]
    print("Building the isolated scale executable; no measurements run during build", flush=True)
    output = assignment.capture(cmd, ROOT)
    binaries = [record["executable"] for line in output.splitlines()
                if (record := json.loads(line)).get("reason") == "compiler-artifact"
                and record.get("target", {}).get("name") == "scale_cost"
                and "example" in record.get("target", {}).get("kind", []) and record.get("executable")]
    if len(binaries) != 1:
        raise ValueError("Cargo did not report a unique scale executable")
    binary = pathlib.Path(binaries[0])
    meta.update(binary_path=str(binary), binary_sha256=assignment.digest(binary), build_command=cmd)
    verify(meta)
    assignment.write_new(out / "build.json", meta)
    print("Build recorded. Stop concurrent builds/tests before starting measurement.", flush=True)


def measure(out):
    if set(path.name for path in out.iterdir()) != {"build.json"}:
        raise ValueError("scale measurement destination must contain only build.json")
    meta = json.loads((out / "build.json").read_text(encoding="utf8"))
    verify(meta)
    grouped, execution = {}, []
    for run in range(1, RUNS + 1):
        stamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
        raw = assignment.capture([meta["binary_path"]], ROOT)
        rows = validate(raw)
        verify(meta)
        path = out / f"run-{run}.jsonl"
        # Explicit LF keeps recorded raw hashes stable in the result archive.
        with path.open("x", encoding="utf8", newline="\n") as stream:
            stream.write(raw + "\n")
        execution.append({"run": run, "started_utc": stamp, "file": path.name,
                          "sha256": assignment.digest(path), "command": [meta["binary_path"]]})
        for row in rows:
            key = (row["size"], row["family"], row["one_to_one"])
            grouped.setdefault(key, []).append(row["elapsed_ns"] / row["iterations"] / 1000000)
        print(f"Scale process {run}/{RUNS} completed", flush=True)
    verify(meta)
    summaries = [{"size": size, "family": family, "one_to_one": policy,
                  "process_means_ms": values, "median_ms": statistics.median(values),
                  "min_ms": min(values), "max_ms": max(values)}
                 for (size, family, policy), values in sorted(grouped.items())]
    assignment.write_new(out / "summary.json", {"protocol": PROTOCOL, "execution": execution, "rows": summaries,
                         "machine": assignment.machine(), "cli": [sys.executable, str(pathlib.Path(__file__).resolve()), *sys.argv[1:]],
                         "limitations": "One candidate and fixed synthetic diagonal graphs; five independent process means, no confidence intervals or CPU affinity. Built-in samples are ideal disjoint identifiers. No candidate pruning, accuracy gain, worst-case timing, allocation or peak-memory claim; caller limits are explicitly raised and library defaults remain unchanged. Pair/signal counts describe the full configured product, not instrumentation. Scoped local hashes are not a signed or hermetic-build attestation."})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["build", "run"])
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args()
    (build if args.action == "build" else measure)(args.output.resolve())


if __name__ == "__main__":
    main()
