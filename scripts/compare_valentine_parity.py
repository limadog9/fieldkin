"""Compare actual Rust/Python matcher runs on identical checked-in datasets.

This script does not run a matcher or approximate a reference score. It verifies
input hashes and column identities, compares complete correspondence sets, and
reports exact floating-point equality separately from tolerance equivalence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import sys
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
PAIR_FIELDS = ("source_table", "source_column", "target_table", "target_column")
ALGORITHMS = ("coma", "coma_instances", "cupid", "distribution", "flooding", "jaccard")
EXPECTED_UPSTREAM_REVISION = "d8fa9ee6d312b49ce2c45b62824df1fd31b94a72"
EXPECTED_REFERENCE_PATCHES = (
    "scripts/reference_patches/cupid.patch",
    "scripts/reference_patches/distribution.patch",
)
TOLERANCES = {algorithm: (1e-6 if algorithm.startswith("coma") else 1e-10)
              for algorithm in ALGORITHMS}


def path_key(value: str) -> str:
    value = value.replace("\\", "/")
    return value[2:] if value.startswith("./") else value


def index_datasets(document: dict[str, Any], side: str) -> dict[str, dict[str, Any]]:
    datasets: dict[str, dict[str, Any]] = {}
    for dataset in document["datasets"]:
        key = path_key(dataset["path"])
        if key in datasets:
            raise ValueError(f"{side}: duplicate dataset path {key!r}")
        datasets[key] = dataset
    return datasets


def pair_dict(key: tuple[str, ...]) -> dict[str, str]:
    return dict(zip(PAIR_FIELDS, key))


def package_sha256(source_root: Path) -> str:
    """Independently verify the package digest format used by the reference helper."""
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


def reference_provenance(document: dict[str, Any]) -> dict[str, Any]:
    reference = document.get("reference")
    if reference is None:
        return {
            "mode": "legacy_upstream",
            "status": "unpatched upstream, legacy report without package or patch hashes",
            "verified": document.get("upstream_revision") == EXPECTED_UPSTREAM_REVISION,
            "package_hashes_recorded": False,
            "patches": [],
            "issues": [],
        }
    if not isinstance(reference, dict):
        return {"mode": "invalid", "status": "invalid reference metadata", "verified": False,
                "package_hashes_recorded": False, "patches": [],
                "issues": ["reference metadata must be an object"]}
    mode = reference.get("mode")
    issues = []
    if mode not in ("upstream", "fixed"):
        issues.append(f"unknown reference mode {mode!r}")
    if reference.get("upstream_revision") != EXPECTED_UPSTREAM_REVISION:
        issues.append("reference metadata is not pinned to the expected upstream revision")
    if document.get("upstream_revision") != reference.get("upstream_revision"):
        issues.append("report and reference metadata disagree on the upstream revision")
    for field in ("base_source_sha256", "source_sha256"):
        value = reference.get(field)
        if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
            issues.append(f"{field} must be a lowercase SHA256 digest")
    actual_base_hash = None
    actual_source_hash = None
    base = ROOT / ".upstream/valentine"
    try:
        actual_base_hash = package_sha256(base)
        if actual_base_hash != reference.get("base_source_sha256"):
            issues.append("base package hash differs from the current original checkout")
    except (OSError, ValueError) as error:
        issues.append(f"cannot verify original checkout: {error}")
    patches = reference.get("patches")
    if not isinstance(patches, list):
        issues.append("reference patches must be a list")
        patches = []
    paths = [patch.get("path") for patch in patches if isinstance(patch, dict)]
    expected_paths = list(EXPECTED_REFERENCE_PATCHES) if mode == "fixed" else []
    if (len(paths) != len(patches) or not all(isinstance(path, str) for path in paths)
            or sorted(paths) != sorted(expected_paths)):
        issues.append("fixed mode requires exactly the two known patches; upstream mode requires no patches")
    checked_patches = []
    patch_data = {}
    for patch in patches:
        if not isinstance(patch, dict):
            continue
        path = patch.get("path")
        entry = {"path": path, "recorded_sha256": patch.get("sha256"), "actual_sha256": None,
                 "verified": False}
        if path not in EXPECTED_REFERENCE_PATCHES:
            issues.append(f"unexpected reference patch {path!r}")
        else:
            try:
                data = (ROOT / path).read_bytes()
                actual_patch_hash = hashlib.sha256(data).hexdigest()
                entry["actual_sha256"] = actual_patch_hash
                entry["verified"] = actual_patch_hash == patch.get("sha256")
                patch_data[path] = data
                if not entry["verified"]:
                    issues.append(f"reference patch hash differs from the current file: {path}")
            except OSError as error:
                issues.append(f"cannot verify patch {path}: {error}")
        checked_patches.append(entry)
    source_root = reference.get("source_root")
    try:
        if not isinstance(source_root, str):
            raise ValueError("source_root must be a workspace-relative path")
        source = (ROOT / source_root).resolve()
        source.relative_to(ROOT)
        if mode == "upstream" and source != base.resolve():
            issues.append("upstream mode must use the original checkout")
        if mode == "fixed":
            source.relative_to((ROOT / "target/parity/reference").resolve())
            if actual_base_hash and all(path in patch_data for path in EXPECTED_REFERENCE_PATCHES):
                identity = hashlib.sha256(actual_base_hash.encode("ascii"))
                for path in EXPECTED_REFERENCE_PATCHES:
                    identity.update(path.encode("utf-8") + b"\0" + patch_data[path])
                if source.name != identity.hexdigest():
                    issues.append("fixed source directory is not keyed by the current base package and patches")
        actual_source_hash = package_sha256(source)
        if actual_source_hash != reference.get("source_sha256"):
            issues.append("reference source package hash differs from the current overlay contents")
    except (OSError, ValueError) as error:
        issues.append(f"cannot verify source package: {error}")
    verified = not issues
    label = ("fixed reference with two versioned source fixes" if mode == "fixed"
             else "unpatched upstream")
    return {
        "mode": mode,
        "status": f"{label}; provenance {'verified' if verified else 'FAILED'}",
        "verified": verified,
        "package_hashes_recorded": True,
        "source_root": source_root,
        "recorded_base_source_sha256": reference.get("base_source_sha256"),
        "actual_base_source_sha256": actual_base_hash,
        "recorded_source_sha256": reference.get("source_sha256"),
        "actual_source_sha256": actual_source_hash,
        "patches": checked_patches,
        "issues": issues,
    }


def identifier_issues(
    entries: dict[tuple[str, ...], float], source_names: list[str], target_names: list[str],
) -> list[dict[str, Any]]:
    issues = []
    for key in sorted(entries):
        invalid = []
        if key[0] != "source":
            invalid.append("source_table")
        if key[1] not in source_names:
            invalid.append("source_column")
        if key[2] != "target":
            invalid.append("target_table")
        if key[3] not in target_names:
            invalid.append("target_column")
        if invalid:
            issues.append({"pair": pair_dict(key), "invalid_fields": invalid})
    return issues


def matches(result: dict[str, Any] | None, side: str) -> tuple[dict[tuple[str, ...], float], list[str]]:
    if result is None:
        return {}, [f"{side}: missing algorithm result"]
    issues = [f"{side}: missing {field} field" for field in ("matches", "error") if field not in result]
    entries = {}
    for match in result.get("matches", []):
        key = tuple(match["pair"][field] for field in PAIR_FIELDS)
        if not all(isinstance(value, str) for value in key):
            raise ValueError(f"{side}: correspondence identifiers must be strings")
        value = match["score"]
        if isinstance(value, bool) or not isinstance(value, (float, int)):
            raise ValueError(f"{side}: score must be numeric for {key!r}")
        score = float(value)
        if not math.isfinite(score):
            raise ValueError(f"{side}: score must be finite for {key!r}")
        if key in entries:
            issues.append(f"{side}: duplicate correspondence {key!r}")
        entries[key] = score
    return entries, issues


def compare_case(
    path: str, algorithm: str, rust: dict[str, Any] | None,
    python: dict[str, Any] | None, input_checks: dict[str, Any], context_verified: bool,
) -> dict[str, Any]:
    rust_result = rust.get("algorithms", {}).get(algorithm) if rust else None
    python_result = python.get("algorithms", {}).get(algorithm) if python else None
    rust_matches, rust_issues = matches(rust_result, "Rust")
    python_matches, python_issues = matches(python_result, "Python")
    identity_issues = {
        "rust": identifier_issues(rust_matches, input_checks["source_column_names"], input_checks["target_column_names"]),
        "python": identifier_issues(python_matches, input_checks["source_column_names"], input_checks["target_column_names"]),
    }
    identifiers_valid = not any(identity_issues.values())
    tolerance = TOLERANCES.get(algorithm, 1e-10)
    missing = sorted(python_matches.keys() - rust_matches.keys())
    extra = sorted(rust_matches.keys() - python_matches.keys())
    shared = sorted(rust_matches.keys() & python_matches.keys())
    score_differences = []
    for key in shared:
        error = abs(rust_matches[key] - python_matches[key])
        if rust_matches[key] != python_matches[key]:
            score_differences.append({
                "pair": pair_dict(key), "rust_score": rust_matches[key],
                "python_score": python_matches[key], "absolute_error": error,
                "within_tolerance": error <= tolerance,
            })
    errors = {
        "rust": rust_result.get("error") if rust_result else "missing algorithm result",
        "python": python_result.get("error") if python_result else "missing algorithm result",
    }
    issues = rust_issues + python_issues
    both_succeeded = all(error is None for error in errors.values()) and not issues
    pair_sets_equal = not missing and not extra
    exact_scores = not score_differences
    within_tolerance_scores = all(difference["within_tolerance"] for difference in score_differences)
    return {
        "path": path,
        "name": (rust or python or {}).get("name"),
        "algorithm": algorithm,
        "absolute_tolerance": tolerance,
        "inputs_verified": input_checks["verified"],
        "context_verified": context_verified,
        "identifiers_valid": identifiers_valid,
        "identifier_issues": identity_issues,
        "both_succeeded": both_succeeded,
        "errors": errors,
        "schema_issues": issues,
        "rust_pair_count": len(rust_matches),
        "python_pair_count": len(python_matches),
        "shared_pair_count": len(shared),
        "missing_from_rust": [pair_dict(key) for key in missing],
        "extra_in_rust": [pair_dict(key) for key in extra],
        "shared_score_differences": score_differences,
        "max_absolute_error": max((difference["absolute_error"] for difference in score_differences), default=0.0),
        "pair_sets_equal": pair_sets_equal,
        "exact_scores": exact_scores,
        "within_tolerance_scores": within_tolerance_scores,
        "exact": bool(identifiers_valid and context_verified and input_checks["verified"] and both_succeeded and pair_sets_equal and exact_scores),
        "equivalent": bool(identifiers_valid and context_verified and input_checks["verified"] and both_succeeded and pair_sets_equal and within_tolerance_scores),
        "elapsed_seconds": {
            "rust": rust_result.get("elapsed_seconds") if rust_result else None,
            "python": python_result.get("elapsed_seconds") if python_result else None,
        },
    }


def input_checks(
    path: str, rust: dict[str, Any] | None, python: dict[str, Any] | None,
) -> dict[str, Any]:
    hashes = {"rust": rust.get("input_sha256") if rust else None,
              "python": python.get("input_sha256") if python else None}
    sha_valid = all(isinstance(value, str) and re.fullmatch(r"[0-9a-fA-F]{64}", value)
                    for value in hashes.values())
    sha_equal = bool(sha_valid and hashes["rust"].lower() == hashes["python"].lower())
    actual_sha = None
    actual_input_error = None
    source_column_names = []
    target_column_names = []
    try:
        actual_input = (ROOT / path).resolve()
        actual_input.relative_to(ROOT)
        raw = actual_input.read_bytes()
        actual_sha = hashlib.sha256(raw).hexdigest()
        definition = json.loads(raw)
        source_column_names = [field["name"] for field in definition["source"]["fields"]]
        target_column_names = [field["name"] for field in definition["target"]["fields"]]
    except (OSError, ValueError, KeyError, TypeError) as error:
        actual_input_error = str(error)
    checks = {
        "dataset_present_in_both": rust is not None and python is not None,
        "input_sha256": hashes,
        "sha256_valid": bool(sha_valid),
        "sha256_equal": sha_equal,
        "actual_input_sha256": actual_sha,
        "actual_input_error": actual_input_error,
        "source_column_names": source_column_names,
        "target_column_names": target_column_names,
        "sha256_matches_current_input": bool(sha_equal and hashes["rust"].lower() == actual_sha),
        "source_columns_equal": bool(rust is not None and python is not None
                                     and "source_columns" in rust and "source_columns" in python
                                     and rust.get("source_columns") == python.get("source_columns")),
        "target_columns_equal": bool(rust is not None and python is not None
                                     and "target_columns" in rust and "target_columns" in python
                                     and rust.get("target_columns") == python.get("target_columns")),
        "name_equal": bool(rust is not None and python is not None
                           and "name" in rust and "name" in python
                           and rust.get("name") == python.get("name")),
    }
    checks["verified"] = all(checks[key] for key in (
        "dataset_present_in_both", "sha256_equal", "sha256_matches_current_input", "source_columns_equal",
        "target_columns_equal", "name_equal",
    ))
    return checks


def summarize(cases: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "case_count": len(cases),
        "successful_case_count": sum(case["both_succeeded"] for case in cases),
        "verified_input_case_count": sum(case["inputs_verified"] for case in cases),
        "verified_context_case_count": sum(case["context_verified"] for case in cases),
        "valid_identifiers_case_count": sum(case["identifiers_valid"] for case in cases),
        "equal_pair_set_case_count": sum(case["pair_sets_equal"] for case in cases),
        "exact_case_count": sum(case["exact"] for case in cases),
        "equivalent_case_count": sum(case["equivalent"] for case in cases),
        "failure_count": sum(not case["equivalent"] for case in cases),
        "missing_from_rust_count": sum(len(case["missing_from_rust"]) for case in cases),
        "extra_in_rust_count": sum(len(case["extra_in_rust"]) for case in cases),
        "different_shared_score_count": sum(len(case["shared_score_differences"]) for case in cases),
        "shared_score_count": sum(case["shared_pair_count"] for case in cases),
        "scores_outside_tolerance_count": sum(
            sum(not difference["within_tolerance"] for difference in case["shared_score_differences"])
            for case in cases
        ),
        "max_absolute_error": max((case["max_absolute_error"] for case in cases), default=0.0),
        "all_exact": bool(cases) and all(case["exact"] for case in cases),
        "all_equivalent": bool(cases) and all(case["equivalent"] for case in cases),
    }


def compare(rust: dict[str, Any], python: dict[str, Any]) -> dict[str, Any]:
    rust_datasets = index_datasets(rust, "Rust")
    python_datasets = index_datasets(python, "Python")
    provenance = reference_provenance(python)
    metadata = {
        "rust_version": rust.get("version"), "python_version": python.get("python_version", python.get("version")),
        "upstream_revision": {"rust": rust.get("upstream_revision"), "python": python.get("upstream_revision")},
        "upstream_revision_equal": (rust.get("upstream_revision") == python.get("upstream_revision")
                                    if rust.get("upstream_revision") else None),
        "expected_upstream_revision": EXPECTED_UPSTREAM_REVISION,
        "python_reference_revision_verified": python.get("upstream_revision") == EXPECTED_UPSTREAM_REVISION,
        "reference_status": provenance["status"],
        "reference_provenance": provenance,
        "reference_provenance_verified": provenance["verified"],
        "dtype_mapping": {"rust": rust.get("dtype_mapping"), "python": python.get("dtype_mapping")},
        "dtype_mapping_equal": rust.get("dtype_mapping") == python.get("dtype_mapping"),
        "dtype_mapping_present": all(isinstance(document.get("dtype_mapping"), dict)
                                     and document["dtype_mapping"] for document in (rust, python)),
        "datatype_canonicalization": (
            "The runs use their recorded declared-type mappings to canonicalize Rust's declared "
            "types into Valentine's varchar/int/float/date vocabulary; agreement therefore "
            "validates matching after canonicalization, rather than independent type inference."
        ),
    }
    metadata["context_verified"] = all(metadata[key] for key in (
        "python_reference_revision_verified", "reference_provenance_verified", "dtype_mapping_present", "dtype_mapping_equal",
    ))
    expected_paths = {path_key(str(path.relative_to(ROOT)))
                      for directory in ("eval", "eval_realworld") for path in (ROOT / directory).glob("*.json")}
    coverage = {
        "expected_dataset_count": len(expected_paths),
        "expected_case_count": len(expected_paths) * len(ALGORITHMS),
        "expected_paths": sorted(expected_paths),
        "missing_from_rust": sorted(expected_paths - rust_datasets.keys()),
        "missing_from_python": sorted(expected_paths - python_datasets.keys()),
        "unexpected_in_rust": sorted(rust_datasets.keys() - expected_paths),
        "unexpected_in_python": sorted(python_datasets.keys() - expected_paths),
    }
    coverage["complete"] = bool(expected_paths) and all(not coverage[key] for key in (
        "missing_from_rust", "missing_from_python", "unexpected_in_rust", "unexpected_in_python",
    ))
    checks_by_path = {}
    cases = []
    for path in sorted(expected_paths | rust_datasets.keys() | python_datasets.keys()):
        rust_dataset = rust_datasets.get(path)
        python_dataset = python_datasets.get(path)
        checks = input_checks(path, rust_dataset, python_dataset)
        checks_by_path[path] = checks
        algorithms = set(ALGORITHMS)
        for dataset in (rust_dataset, python_dataset):
            if dataset:
                algorithms.update(dataset.get("algorithms", {}))
        for algorithm in sorted(algorithms):
            cases.append(compare_case(path, algorithm, rust_dataset, python_dataset, checks, metadata["context_verified"]))
    algorithm_names = sorted({case["algorithm"] for case in cases})
    report = {
        "version": 2,
        "method": {
            "pair_sets": "exact equality of all four table/column identifiers",
            "score_exactness": "numeric equality of parsed IEEE-754 binary64 values, without tolerance",
            "score_equivalence": "absolute error at most the algorithm's recorded tolerance",
            "absolute_tolerances": TOLERANCES,
            "identifier_integrity": "every returned identifier must exist in the original input schema; names are never normalized",
            "error_handling": "algorithm errors, duplicate pairs, invalid identifiers and unverified inputs/provenance fail equivalence",
        },
        "metadata": metadata,
        "coverage": coverage,
        "dataset_count": len(checks_by_path),
        "input_checks": checks_by_path,
        "aggregate": summarize(cases),
        "algorithms": {
            algorithm: summarize([case for case in cases if case["algorithm"] == algorithm])
            for algorithm in algorithm_names
        },
        "cases": cases,
    }
    report["aggregate"]["all_equivalent"] &= coverage["complete"]
    report["aggregate"]["all_exact"] &= coverage["complete"]
    return report


def print_summary(report: dict[str, Any], output: Path) -> None:
    print(f"Reference: {report['metadata']['reference_status']}")
    print("Algorithm         Cases  Inputs  Pairs  Exact  Equivalent  Missing  Extra  Max score error")
    for algorithm, summary in report["algorithms"].items():
        print(f"{algorithm:<17} {summary['case_count']:>5} "
              f"{summary['verified_input_case_count']:>7} "
              f"{summary['equal_pair_set_case_count']:>6} "
              f"{summary['exact_case_count']:>6} "
              f"{summary['equivalent_case_count']:>11} "
              f"{summary['missing_from_rust_count']:>8} "
              f"{summary['extra_in_rust_count']:>6} "
              f"{summary['max_absolute_error']:.12g}")
    for path, checks in report["input_checks"].items():
        if not checks["verified"]:
            failed = [key for key, value in checks.items()
                      if isinstance(value, bool) and not value and key != "verified"]
            print(f"INPUT FAILURE {path}: {', '.join(failed)}")
    if not report["metadata"]["context_verified"]:
        print("CONTEXT FAILURE: reference revision/provenance or canonical dtype mapping is missing or differs.")
        for issue in report["metadata"]["reference_provenance"]["issues"]:
            print(f"REFERENCE FAILURE: {issue}")
    if not report["coverage"]["complete"]:
        print(f"COVERAGE FAILURE: {report['coverage']}")
    for case in report["cases"]:
        if not case["equivalent"]:
            outside = sum(not difference["within_tolerance"]
                          for difference in case["shared_score_differences"])
            print(f"FAIL {case['path']} / {case['algorithm']}: "
                  f"missing={len(case['missing_from_rust'])} extra={len(case['extra_in_rust'])} "
                  f"scores_outside_tolerance={outside} max_error={case['max_absolute_error']:.12g} "
                  f"inputs_verified={case['inputs_verified']} errors={case['errors']} "
                  f"schema_issues={case['schema_issues']} identifiers_valid={case['identifiers_valid']}")
    print(f"Declared-type canonicalization recorded; dtype mappings equal: "
          f"{report['metadata']['dtype_mapping_equal']}.")
    print(f"Result: {report['aggregate']['equivalent_case_count']}/"
          f"{report['aggregate']['case_count']} equivalent; "
          f"{report['aggregate']['exact_case_count']} exactly equal. Report: {output}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rust", type=Path, default=ROOT / "target/parity/rust_results.json")
    parser.add_argument("--python", type=Path, default=ROOT / "target/parity/python_results.json")
    parser.add_argument("--output", type=Path, default=ROOT / "target/parity/comparison.json")
    arguments = parser.parse_args()
    try:
        rust = json.loads(arguments.rust.read_text(encoding="utf-8-sig"))
        python = json.loads(arguments.python.read_text(encoding="utf-8-sig"))
        report = compare(rust, python)
        if report["metadata"]["reference_provenance"]["mode"] == "fixed" and arguments.output.resolve() == (ROOT / "target/parity/comparison.json").resolve():
            raise ValueError("Fixed-reference comparisons require an explicit separate --output; comparison.json preserves the original upstream baseline")
    except (OSError, ValueError, TypeError, KeyError) as error:
        print(f"Cannot compare matcher results: {error}", file=sys.stderr)
        return 2
    report["result_files"] = {"rust": str(arguments.rust), "python": str(arguments.python)}
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, ensure_ascii=False, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    print_summary(report, arguments.output)
    return 0 if report["aggregate"]["all_equivalent"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
