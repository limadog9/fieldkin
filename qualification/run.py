#!/usr/bin/env python3
"""Build and execute a versioned qualification campaign; Python 3.9+, no packages."""
import argparse
import datetime
import hashlib
import json
import os
import pathlib
import platform
import subprocess
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
TOOLCHAIN = "1.85.0"

def command(args):
    return subprocess.run([str(value) for value in args], cwd=ROOT, check=True,
                          capture_output=True, text=True).stdout.strip()

def digest(path, binary=False):
    data = path.read_bytes()
    return hashlib.sha256(data if binary else data.replace(b"\r\n", b"\n")).hexdigest()

def sources():
    files = [ROOT / "Cargo.toml", ROOT / "Cargo.lock"] + sorted((ROOT / "src").glob("*.rs"))
    files += [ROOT / "qualification" / name for name in ["Cargo.toml", "Cargo.lock", "run.py", "README.md"]]
    files += sorted((ROOT / "qualification/src").glob("*.rs"))
    return {str(path.relative_to(ROOT)).replace("\\", "/"): digest(path) for path in files}

def build_environment():
    keys = {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_TARGET"}
    return {key:value for key,value in sorted(os.environ.items()) if key in keys or key.startswith("CARGO_PROFILE_RELEASE_")}

def write_new(path, value):
    with path.open("x", encoding="utf8") as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write("\n")

def build(output):
    output.mkdir(parents=True, exist_ok=True)
    if (output / "build.json").exists():
        raise ValueError("build.json exists; choose a new output directory")
    before = sources()
    invocation = ["cargo", "+"+TOOLCHAIN, "build", "--release", "--locked", "--manifest-path", "qualification/Cargo.toml", "--message-format=json-render-diagnostics"]
    compiled = subprocess.run(invocation, cwd=ROOT, check=True, capture_output=True, text=True)
    if before != sources():
        raise ValueError("sources changed during compilation; rebuild into a fresh output directory")
    artifacts = [json.loads(line) for line in compiled.stdout.splitlines() if line.strip()]
    executables = [entry["executable"] for entry in artifacts
                   if entry.get("reason") == "compiler-artifact"
                   and entry.get("target", {}).get("name") == "fieldkin-qualification"
                   and entry.get("executable")]
    if len(executables) != 1:
        raise ValueError("Cargo did not report exactly one qualification executable")
    binary = pathlib.Path(executables[0]).resolve()
    # Cargo's artifact path handles target directories and explicit target triples.
    # An absolute external target directory is supported without guessing its layout.
    try:
        binary_path = str(binary.relative_to(ROOT)).replace("\\", "/")
    except ValueError:
        binary_path = str(binary)
    write_new(output / "build.json", {
        "protocol":"fieldkin-qualification-v1", "built_utc":datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "commit":command(["git", "rev-parse", "HEAD"]), "git_status":command(["git", "status", "--short"]),
        "compiler":command(["rustc", "+"+TOOLCHAIN, "-Vv"]), "source_sha256":before,
        "build_command":invocation, "build_environment":build_environment(),
        "binary_path":binary_path, "binary_sha256":digest(binary, True),
        "machine":{"platform":platform.platform(), "architecture":platform.machine(), "processor":platform.processor(),
                   "logical_cpus":os.cpu_count(), "python":platform.python_version()},
    })
    print("Qualification binary built. Freeze sources before running the campaign.", flush=True)

def run(output, cases, seed):
    if (output / "run.json").exists():
        raise ValueError("run.json exists; choose a new output directory")
    record = json.loads((output / "build.json").read_text(encoding="utf8"))
    binary = ROOT / record["binary_path"]
    if record["source_sha256"] != sources() or record["binary_sha256"] != digest(binary, True):
        raise ValueError("sources or binary changed after build; rebuild in a new output directory")
    if record["compiler"] != command(["rustc", "+"+TOOLCHAIN, "-Vv"]) or record["build_environment"] != build_environment():
        raise ValueError("compiler/build environment changed after build")
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    before = time.perf_counter()
    completed = subprocess.run([str(binary), "--cases", str(cases), "--seed", str(seed)], cwd=ROOT,
                               capture_output=True, text=True, timeout=600)
    elapsed = time.perf_counter() - before
    if record["source_sha256"] != sources():
        raise ValueError("sources changed during campaign; rerun from a frozen revision")
    if record["binary_sha256"] != digest(binary, True):
        raise ValueError("binary changed during campaign; rebuild and rerun")
    result = {"started_utc":started, "wall_seconds":elapsed, "exit_code":completed.returncode,
              "requested_cases":cases, "seed":seed, "stderr":completed.stderr,
              "build_sha256":digest(output / "build.json")}
    if completed.returncode == 0:
        campaign = json.loads(completed.stdout)
        if campaign["cases"] != cases or sum(campaign["categories"].values()) != cases or not campaign["passed"]:
            raise ValueError("campaign counts/status inconsistent")
        result["campaign"] = campaign
    else:
        result["stdout"] = completed.stdout
    write_new(output / "run.json", result)
    print(json.dumps(result.get("campaign", result), indent=2), flush=True)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["build", "run"])
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--cases", type=int, default=1_000_000)
    parser.add_argument("--seed", type=int, default=20_261_003)
    args = parser.parse_args()
    if not 1 <= args.cases <= 10_000_000 or not 0 <= args.seed <= 2**64-1:
        parser.error("cases must be 1..=10000000 and seed must fit u64")
    if args.action == "build": build(args.output.resolve())
    else: run(args.output.resolve(), args.cases, args.seed)

if __name__ == "__main__": main()
