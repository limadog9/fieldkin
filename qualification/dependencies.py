#!/usr/bin/env python3
"""Capture all locked dependency licenses and reproduce the dated RustSec scan."""
import argparse
import datetime
import hashlib
import json
import pathlib
import subprocess
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
MANIFESTS = ["Cargo.toml", "performance/Cargo.toml", "qualification/Cargo.toml"]

def command(args):
    return subprocess.run([str(value) for value in args], cwd=ROOT, check=True,
                          capture_output=True, text=True).stdout.strip()

def digest(path, binary=False):
    data = path.read_bytes()
    return hashlib.sha256(data if binary else data.replace(b"\r\n", b"\n")).hexdigest()

def fetch(url):
    request = urllib.request.Request(url, headers={"User-Agent":"fieldkin-dependency-review/0.1 (https://github.com/limadog9/fieldkin)", "Accept":"application/json"})
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)

def capture(output, audit, database):
    packages = {}
    for manifest in MANIFESTS:
        metadata = json.loads(command(["cargo", "+1.85.0", "metadata", "--locked", "--format-version", "1", "--manifest-path", manifest, "--all-features"]))
        for package in metadata["packages"]:
            if package["source"] is None:
                continue
            key = package["name"]+"@"+package["version"]
            if key not in packages:
                directory = pathlib.Path(package["manifest_path"]).parent
                notices = [path for path in directory.iterdir() if path.is_file() and (path.name.upper().startswith("LICENSE") or path.name.upper().startswith("COPYING") or path.name.upper().startswith("COPYRIGHT"))]
                packages[key] = {
                    "name":package["name"], "version":package["version"], "license":package["license"],
                    "repository":package["repository"], "source":package["source"], "graphs":[],
                    "license_file":package["license_file"],
                    "notice_sha256":{path.name:digest(path) for path in sorted(notices)},
                    "crate_manifest_sha256":digest(directory / "Cargo.toml"),
                }
            packages[key]["graphs"].append(manifest)
    direct = ["strsim", "unicode-normalization", "tinyvec", "proptest", "serde", "serde_json", "sha2", "stats_alloc"]
    upstream = {}
    for name in direct:
        result = fetch("https://crates.io/api/v1/crates/"+name)
        crate = result["crate"]
        selected = [package for package in packages.values() if package["name"] == name]
        version_records = []
        for package in selected:
            version = next(version for version in result["versions"] if version["num"] == package["version"])
            version_records.append({key:version[key] for key in ["num", "created_at", "updated_at", "yanked", "license"]})
        entry = {"crate_url":"https://crates.io/crates/"+name, "latest_version":crate["max_version"],
                 "last_updated":crate["updated_at"], "locked_versions":version_records, "repository":crate["repository"]}
        if crate["repository"] and crate["repository"].startswith("https://github.com/"):
            path = crate["repository"].removeprefix("https://github.com/").removesuffix(".git").rstrip("/")
            if len(path.split("/")) == 2:
                try:
                    repository = fetch("https://api.github.com/repos/"+path)
                    entry["github"] = {key:repository[key] for key in ["html_url", "archived", "disabled", "pushed_at", "updated_at"]}
                except Exception as error:
                    entry["github_error"] = type(error).__name__
        upstream[name] = entry
    records = {}
    for manifest in MANIFESTS:
        lock = pathlib.Path(manifest).with_name("Cargo.lock")
        invocation = [str(audit), "audit", "--json", "--db", str(database), "--no-fetch", "--file", str(lock), "--deny", "warnings"]
        scanned = subprocess.run(invocation, cwd=ROOT, capture_output=True, text=True)
        recorded_command = ["cargo-audit", "audit", "--json", "--db", "<advisory-db-checkout>", "--no-fetch", "--file", str(lock).replace("\\", "/"), "--deny", "warnings"]
        records[str(lock).replace("\\", "/")] = {"command":recorded_command, "exit_code":scanned.returncode,
            "lock_sha256":digest(ROOT / lock), "stderr":scanned.stderr, "report":json.loads(scanned.stdout)}
    capture = {"reviewed_utc":datetime.datetime.now(datetime.timezone.utc).isoformat(), "git_commit":command(["git", "rev-parse", "HEAD"]),
        "tool":command([audit, "audit", "--version"]), "tool_binary_sha256":digest(audit, True),
        "advisory_database":{"url":"https://github.com/rustsec/advisory-db", "commit":command(["git", "-C", database, "rev-parse", "HEAD"]),
            "commit_time":command(["git", "-C", database, "log", "-1", "--format=%cI"])},
        "packages":dict(sorted(packages.items())), "upstream":upstream, "audits":records}
    with output.open("x", encoding="utf8") as handle:
        json.dump(capture, handle, indent=2, sort_keys=True)
        handle.write("\n")
    print(f"Recorded {len(packages)} third-party package versions across three lockfiles")
    if any(record["exit_code"] != 0 for record in records.values()):
        raise SystemExit("At least one advisory audit did not pass; inspect the saved report")

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--audit", type=pathlib.Path, required=True)
    parser.add_argument("--database", type=pathlib.Path, required=True)
    arguments = parser.parse_args()
    capture(arguments.output.resolve(), arguments.audit.resolve(), arguments.database.resolve())
