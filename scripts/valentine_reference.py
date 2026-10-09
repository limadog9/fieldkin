"""Prepare the pinned reference, optionally applying two auditable source fixes.

The original checkout is never modified. Fixed runs import a separate source
copy and record its contents and patch hashes in their output metadata.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
EXPECTED_REVISION = "d8fa9ee6d312b49ce2c45b62824df1fd31b94a72"
PATCHES = (
    ("scripts/reference_patches/cupid.patch", "valentine/algorithms/cupid/linguistic_matching.py"),
    ("scripts/reference_patches/distribution.patch", "valentine/algorithms/distribution_based/discovery.py"),
)


def package_sha256(source_root: Path) -> str:
    """Hash package paths and contents, excluding generated Python bytecode."""
    digest = hashlib.sha256()
    package = source_root / "valentine"
    paths = sorted(path for path in package.rglob("*")
                   if path.is_file() and "__pycache__" not in path.parts
                   and path.suffix not in (".pyc", ".pyo"))
    if not paths:
        raise ValueError(f"Missing Valentine package in {source_root}")
    for path in paths:
        digest.update(path.relative_to(source_root).as_posix().encode("utf-8") + b"\0")
        data = path.read_bytes()
        digest.update(len(data).to_bytes(8, "big"))
        digest.update(data)
    return digest.hexdigest()


def prepare_reference(fixes: bool = False) -> tuple[Path, dict]:
    source = ROOT / ".upstream/valentine"
    revision = subprocess.check_output(
        ["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
    if revision != EXPECTED_REVISION:
        raise ValueError(f"Reference must be pinned to {EXPECTED_REVISION}; found {revision}")
    # Refuse to label a modified checkout as the original reference.
    subprocess.run(["git", "-C", str(source), "diff", "--exit-code", "HEAD", "--", "valentine"],
                   check=True, capture_output=True)
    untracked = subprocess.check_output(
        ["git", "-C", str(source), "ls-files", "--others", "--exclude-standard", "--", "valentine"],
        text=True).splitlines()
    if untracked:
        raise ValueError(f"Untracked files in the reference package: {untracked}")
    base_hash = package_sha256(source)
    metadata = {
        "mode": "fixed" if fixes else "upstream",
        "upstream_revision": revision,
        "base_source_sha256": base_hash,
        "source_sha256": base_hash,
        "source_root": source.relative_to(ROOT).as_posix(),
        "patches": [],
    }
    if not fixes:
        return source, metadata

    identity = hashlib.sha256(base_hash.encode("ascii"))
    for patch_path, target in PATCHES:
        patch = ROOT / patch_path
        data = patch.read_bytes()
        # Restrict every patch to its declared Python source file.
        headers = [line for line in data.decode("utf-8").splitlines()
                   if line.startswith(("--- ", "+++ "))]
        if headers != [f"--- a/{target}", f"+++ b/{target}"]:
            raise ValueError(f"Unexpected source paths in {patch_path}: {headers}")
        patch_hash = hashlib.sha256(data).hexdigest()
        metadata["patches"].append({"path": patch_path, "sha256": patch_hash})
        identity.update(patch_path.encode("utf-8") + b"\0" + data)

    overlay = (ROOT / "target/parity/reference" / identity.hexdigest()).resolve()
    overlay.relative_to(ROOT / "target/parity/reference")
    marker = overlay / "reference.json"
    if marker.exists():
        recorded = json.loads(marker.read_text(encoding="utf-8"))
        if (recorded["base_source_sha256"] != base_hash
                or recorded["patches"] != metadata["patches"]
                or recorded["source_sha256"] != package_sha256(overlay)):
            raise ValueError(f"Prepared reference has changed: {overlay}")
        return overlay, recorded
    if overlay.exists():
        raise ValueError(f"Incomplete reference source copy: {overlay}")
    overlay.mkdir(parents=True)
    try:
        shutil.copytree(source / "valentine", overlay / "valentine",
                        ignore=shutil.ignore_patterns("__pycache__", "*.pyc", "*.pyo"))
        relative_overlay = overlay.relative_to(ROOT).as_posix()
        for patch in metadata["patches"]:
            command = ["git", "apply", f"--directory={relative_overlay}",
                       str(ROOT / patch["path"])]
            subprocess.run(command[:2] + ["--check"] + command[2:],
                           cwd=ROOT, check=True, capture_output=True)
            subprocess.run(command, cwd=ROOT, check=True, capture_output=True)
        metadata["source_root"] = overlay.relative_to(ROOT).as_posix()
        metadata["source_sha256"] = package_sha256(overlay)
        marker.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    except Exception:
        # Only remove the incomplete source copy we just created, after checking
        # its absolute location remains inside this task's reference directory.
        checked_overlay = overlay.resolve()
        checked_overlay.relative_to((ROOT / "target/parity/reference").resolve())
        if checked_overlay.exists():
            shutil.rmtree(checked_overlay)
        raise
    return overlay, metadata
