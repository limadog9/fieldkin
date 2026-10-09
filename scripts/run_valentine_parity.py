"""Run upstream Python Valentine on Fieldkin's existing evaluation datasets.

Run from any directory:
    python scripts/run_valentine_parity.py
    python scripts/run_valentine_parity.py --reference-fixes

Requires the pinned checkout at .upstream/valentine, its dependencies at
.upstream/python, and cached NLTK corpora at .upstream/nltk_data. No datasets
are resampled, inferred, or regenerated. This is verification tooling only.
"""

import argparse
import hashlib
import importlib.metadata
import json
import math
from pathlib import Path
import subprocess
import sys
import tempfile
import time

from valentine_reference import prepare_reference

ROOT = Path(__file__).resolve().parent.parent
reference_parser = argparse.ArgumentParser(add_help=False)
reference_parser.add_argument("--reference-fixes", action="store_true")
reference_options, _ = reference_parser.parse_known_args()
REFERENCE_ROOT, REFERENCE_METADATA = prepare_reference(reference_options.reference_fixes)
sys.path[:0] = [str(REFERENCE_ROOT), str(ROOT / ".upstream/python")]

import nltk

nltk.data.path.insert(0, str(ROOT / ".upstream/nltk_data"))

import valentine

IMPORTED_PACKAGE_FILE = Path(valentine.__file__).resolve()
if not IMPORTED_PACKAGE_FILE.is_relative_to(REFERENCE_ROOT.resolve()):
    raise ImportError(f"Valentine imported outside the selected reference: {IMPORTED_PACKAGE_FILE}")

from valentine.algorithms import (
    Coma,
    Cupid,
    DistributionBased,
    JaccardDistanceMatcher,
    SimilarityFlooding,
)
from valentine.data_sources.base_column import BaseColumn
from valentine.data_sources.base_table import BaseTable

DTYPE_MAPPING = {
    "integer": "int",
    "float": "float",
    "decimal": "float",
    "date": "date",
    "timestamp": "date",
    "text": "varchar",
    "unknown": "varchar",
    "boolean": "varchar",
}

MATCHERS = {
    "coma": lambda: Coma(),
    "coma_instances": lambda: Coma(use_instances=True),
    "cupid": lambda: Cupid(),
    "distribution": lambda: DistributionBased(),
    "flooding": lambda: SimilarityFlooding(),
    "jaccard": lambda: JaccardDistanceMatcher(),
}


class InputColumn(BaseColumn):
    def __init__(self, table_name, field, index):
        self._name = field["name"]
        self._uid = f"{table_name}:{index}:{self._name}"
        self._dtype = DTYPE_MAPPING[field["data_type"]]
        self._samples = list(field.get("samples", []))
        if not isinstance(self._name, str) or not all(isinstance(v, str) for v in self._samples):
            raise ValueError("Column names and samples must be strings")

    @property
    def unique_identifier(self):
        return self._uid

    @property
    def name(self):
        return self._name

    @property
    def data_type(self):
        return self._dtype

    @property
    def data(self):
        return self._samples


class InputTable(BaseTable):
    def __init__(self, name, fields):
        self._name = name
        self._columns = [InputColumn(name, field, i) for i, field in enumerate(fields)]

    @property
    def unique_identifier(self):
        return self._name

    @property
    def name(self):
        return self._name

    @property
    def is_empty(self):
        return all(column.is_empty for column in self._columns)

    def get_columns(self):
        return self._columns

    def get_instances_columns(self):
        return self._columns

    def get_df(self):
        raise AssertionError("The live parity runner supplies columns directly; frame inference is disabled")


def format_matches(matches):
    records = []
    for pair, score in matches.items():
        score = float(score)
        if not math.isfinite(score):
            raise ValueError(f"Upstream returned a nonfinite score for {pair}: {score}")
        records.append({
            "pair": {
                "source_table": pair.source_table,
                "source_column": pair.source_column,
                "target_table": pair.target_table,
                "target_column": pair.target_column,
            },
            "score": score,
        })
    records.sort(key=lambda record: tuple(record["pair"][key] for key in (
        "source_table", "source_column", "target_table", "target_column")))
    return records


def save(destination, report):
    destination.write_text(json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-fixes", action="store_true",
                        help="Apply the two recorded source fixes to a separate reference copy")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--dataset", action="append", default=[], help="Optional exact repository-relative dataset path")
    parser.add_argument("--algorithm", action="append", choices=tuple(MATCHERS), default=[])
    args = parser.parse_args()
    output_name = "python_results_resolved.json" if args.reference_fixes else "python_results.json"
    destination = (args.output or ROOT / "target/parity" / output_name).resolve()
    if args.reference_fixes and destination == (ROOT / "target/parity/python_results.json").resolve():
        parser.error("Fixed-reference runs require a separate output; python_results.json preserves the upstream baseline")
    destination.parent.mkdir(parents=True, exist_ok=True)
    # Keep DistributionBased's temporary caches within this verification run.
    temp_directory = destination.parent / f"python_tmp_{destination.stem}"
    temp_directory.mkdir(exist_ok=True)
    tempfile.tempdir = str(temp_directory)

    paths = sorted([*ROOT.glob("eval/*.json"), *ROOT.glob("eval_realworld/*.json")],
                   key=lambda path: path.relative_to(ROOT).as_posix())
    if args.dataset:
        requested = set(args.dataset)
        paths = [path for path in paths if path.relative_to(ROOT).as_posix() in requested]
        missing = requested - {path.relative_to(ROOT).as_posix() for path in paths}
        if missing:
            parser.error(f"Unknown dataset paths: {sorted(missing)}")
    algorithms = args.algorithm or list(MATCHERS)
    revision = subprocess.check_output(
        ["git", "-C", str(ROOT / ".upstream/valentine"), "rev-parse", "HEAD"], text=True).strip()
    report = {
        "upstream_revision": revision,
        "reference": REFERENCE_METADATA,
        "imported_package_file": IMPORTED_PACKAGE_FILE.relative_to(ROOT).as_posix(),
        "dtype_mapping": DTYPE_MAPPING,
        "table_names": {"source": "source", "target": "target"},
        "sampling": "all original sample values, preserving order and duplicates",
        "python_version": sys.version.split()[0],
        "dependency_versions": {name: importlib.metadata.version(name) for name in
                                ("nltk", "numpy", "scipy", "rapidfuzz")},
        "datasets": [],
    }
    total_started = time.perf_counter()
    print(f"Reference: {REFERENCE_METADATA['mode']}; source SHA256 "
          f"{REFERENCE_METADATA['source_sha256']}; patches: "
          f"{[patch['path'] for patch in REFERENCE_METADATA['patches']]}", flush=True)
    for dataset_number, path in enumerate(paths, 1):
        raw = path.read_bytes()
        data = json.loads(raw)
        dataset = {
            "path": path.relative_to(ROOT).as_posix(),
            "name": data.get("name", path.stem),
            "input_sha256": hashlib.sha256(raw).hexdigest(),
            "source_columns": len(data["source"]["fields"]),
            "target_columns": len(data["target"]["fields"]),
            "algorithms": {},
        }
        report["datasets"].append(dataset)
        for algorithm in algorithms:
            started = time.perf_counter()
            try:
                # Fresh adapters and matcher for each run prevent any mutable
                # upstream caches from affecting another configuration's input.
                source = InputTable("source", data["source"]["fields"])
                target = InputTable("target", data["target"]["fields"])
                matches = MATCHERS[algorithm]().get_matches(source, target)
                result = {"matches": format_matches(matches), "error": None}
            except Exception as error:
                result = {"matches": [], "error": {"type": type(error).__name__, "message": str(error)}}
            result["elapsed_seconds"] = time.perf_counter() - started
            dataset["algorithms"][algorithm] = result
            save(destination, report)
            status = f"{len(result['matches'])} pairs" if result["error"] is None else str(result["error"])
            print(f"[{dataset_number}/{len(paths)}] {dataset['path']} {algorithm}: "
                  f"{status} ({result['elapsed_seconds']:.3f}s)", flush=True)
    report["elapsed_seconds"] = time.perf_counter() - total_started
    save(destination, report)
    failures = sum(result["error"] is not None for dataset in report["datasets"]
                   for result in dataset["algorithms"].values())
    print(f"Wrote {len(paths)} datasets x {len(algorithms)} configurations to {destination}; "
          f"upstream errors: {failures}", flush=True)
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
