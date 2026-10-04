#!/usr/bin/env python3
"""Pinned, local COMA comparison. Prepare records inputs; run scores both modes.

This is a local drift guard, not a security sandbox or a hermetic attestation.
No labels are passed to the matcher. Exceptions and third-party output are
suppressed because they could contain input values.
"""

import argparse
import contextlib
import datetime
import hashlib
import importlib.metadata
import json
import math
import numbers
import os
import pathlib
import platform
import re
import socket
import stat
import sys
import urllib.request


ROOT = pathlib.Path(__file__).resolve().parent.parent
PROTOCOL = "fieldkin-valentine-comparison-v1"
MODES = ("schema_only", "schema_and_samples")
ARTIFACTS = {mode: f"valentine-{mode}.json" for mode in MODES}
SETTINGS = {"algorithm": "Coma", "max_n": 0, "delta": 1.0, "threshold": 0.0,
            "use_schema": True, "instance_weight": 1.0, "df_names": ["aaa", "bbb"],
            "instance_sample_size": None, "dtype": "object", "sample_rows": 64}
INPUTS = ("evaluation/fixtures/northix-v1.json", "evaluation/northix-protocol.json",
          "evaluation/compare_valentine.py", "evaluation/valentine-environment.json",
          "evaluation/valentine-requirements.txt")


class VerificationError(ValueError):
    """A fixed input or comparison invariant did not hold."""


def checked(path):
    path = pathlib.Path(os.path.abspath(path))
    for component in (*reversed(path.parents), path):
        if component.exists():
            flags = getattr(component.lstat(), "st_file_attributes", 0)
            if component.is_symlink() or flags & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0):
                raise VerificationError("symlinks and junctions are unsupported")
    return path


def digest(path):
    path = checked(path)
    if not path.is_file():
        raise VerificationError("a recorded input file is missing")
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def load(path, limit=32 * 1024 * 1024):
    path = checked(path)
    if path.stat().st_size > limit:
        raise VerificationError("JSON input exceeds its bound")
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise VerificationError("duplicate JSON key")
            result[key] = value
        return result
    return json.loads(path.read_text(encoding="utf8"), object_pairs_hook=unique)


def canonical_hash(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"),
                                     allow_nan=False).encode("utf8")).hexdigest()


def write_new(path, value):
    with path.open("x", encoding="utf8", newline="\n") as stream:
        json.dump(value, stream, sort_keys=True, indent=2, allow_nan=False)
        stream.write("\n")


def normalized(name):
    return re.sub(r"[-_.]+", "-", name).lower()


def installed_inventory(lock):
    expected = {normalized(entry["name"]): entry["version"] for entry in lock["packages"]}
    expected.update({normalized(name): version for name, version in lock["bootstrap"].items()})
    if expected.get("valentine") != "1.0.0":
        raise VerificationError("environment lock must pin Valentine 1.0.0")
    prefix = checked(sys.prefix)
    result = {}
    for distribution in importlib.metadata.distributions():
        name = normalized(distribution.metadata["Name"])
        if name in result or name not in expected or distribution.version != expected[name]:
            raise VerificationError("installed distributions differ from the environment lock")
        files = {}
        if distribution.files is None:
            raise VerificationError("installed distribution has no file inventory")
        for entry in distribution.files:
            path = checked(distribution.locate_file(entry))
            if path.suffix.lower() == ".pyc" or "__pycache__" in path.parts:
                continue
            if not path.is_relative_to(prefix):
                raise VerificationError("installed distribution file leaves the isolated environment")
            files[path.relative_to(prefix).as_posix()] = digest(path)
        result[name] = {"version": distribution.version, "files_sha256": dict(sorted(files.items()))}
    if set(result) != set(expected):
        raise VerificationError("a locked distribution is missing")
    return dict(sorted(result.items()))


def resource_inventory(nltk_data):
    nltk_data = checked(nltk_data)
    if not nltk_data.is_relative_to(ROOT) or not (nltk_data / "corpora/stopwords/english").is_file():
        raise VerificationError("provide local English stopwords under the workspace")
    result = {}
    for path in sorted(nltk_data.rglob("*")):
        checked(path)
        if path.is_file():
            result[path.relative_to(nltk_data).as_posix()] = digest(path)
    return result


def context(nltk_data):
    lock = load(ROOT / "evaluation/valentine-environment.json")
    if lock["protocol"] != "fieldkin-valentine-environment-v1" or lock["python"] != sys.version or lock["platform"] != platform.platform():
        raise VerificationError("Python or platform differs from the frozen environment")
    if sys.prefix == sys.base_prefix or platform.python_version() != "3.11.4":
        raise VerificationError("use the isolated pinned CPython 3.11.4 environment")
    python = checked(sys.executable)
    base = checked(getattr(sys, "_base_executable", sys.executable))
    resources = resource_inventory(nltk_data)
    expected_resources = {"corpora/" + name: value for name, value in lock["nltk_stopwords"]["files_sha256"].items()}
    if resources != expected_resources:
        raise VerificationError("local stopwords differ from the frozen resource hashes")
    return {"input_sha256": {name: digest(ROOT / name) for name in INPUTS},
            "python": {"version": sys.version, "executable": str(python), "sha256": digest(python),
                       "base_executable": str(base), "base_sha256": digest(base), "prefix": sys.prefix,
                       "sys_path": sys.path, "platform": platform.platform()},
            "distributions": installed_inventory(lock),
            "nltk_data": str(checked(nltk_data)), "resources_sha256": resources,
            "environment_sha256": {name: hashlib.sha256(os.environ[name].encode()).hexdigest()
                                   for name in ("PYTHONHOME", "PYTHONPATH", "PYTHONHASHSEED", "NLTK_DATA",
                                                "OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS")
                                   if name in os.environ}}


def comparison_inputs(fixture, expected_pairs=84):
    """Return only matching inputs. Labels and other fixture metadata are ignored."""
    if fixture.get("format") != "fieldkin-northix-v1":
        raise VerificationError("unexpected fixture format")
    tables = {}
    for table in fixture["tables"]:
        table_id = table["id"]
        if not isinstance(table_id, str) or table_id in tables or not 0 < len(table["fields"]) <= 128:
            raise VerificationError("invalid or duplicate input table")
        fields, names, ids = [], set(), set()
        for field in table["fields"]:
            identity, name, samples = field["id"], field["name"], field["samples"]
            if not isinstance(identity, str) or not isinstance(name, str) or not identity or not name or identity in ids or name in names:
                raise VerificationError("invalid or duplicate field identity/name")
            if not isinstance(samples, list) or len(samples) > 64 or any(value is not None and not isinstance(value, str) for value in samples):
                raise VerificationError("samples must contain at most 64 strings or nulls")
            fields.append({"id": identity, "name": name, "samples": list(samples)})
            names.add(name)
            ids.add(identity)
        tables[table_id] = {"id": table_id, "fields": fields}
    pairs, seen = [], set()
    for pair in fixture["pairs"]:
        identity, source, target = pair["id"], pair["source_table"], pair["target_table"]
        if not isinstance(identity, str) or identity in seen or source not in tables or target not in tables or source == target:
            raise VerificationError("invalid or duplicate table pair")
        seen.add(identity)
        pairs.append({"id": identity, "source_table": source, "target_table": target})
    if len(pairs) != expected_pairs:
        raise VerificationError("comparison must cover the complete fixed pair inventory")
    return tables, sorted(pairs, key=lambda pair: pair["id"])


def columns(table, mode):
    if mode not in MODES:
        raise VerificationError("unknown comparison mode")
    return {field["name"]: ((field["samples"] + [None] * (64 - len(field["samples"])))
                             if mode == "schema_and_samples" else [None] * 64)
            for field in table["fields"]}


def translate_scores(matches, source, target):
    source_names = {field["name"]: field["id"] for field in source["fields"]}
    target_names = {field["name"]: field["id"] for field in target["fields"]}
    scores, seen = [], set()
    for pair, value in matches.items():
        # Valentine1.0's ColumnPair has four named tuple fields. Accept only the
        # specified direction and exact original names; never infer matches.
        if pair.source_table != "aaa" or pair.target_table != "bbb" or pair.source_column not in source_names or pair.target_column not in target_names:
            raise VerificationError("matcher returned an unexpected column identity")
        if isinstance(value, bool) or not isinstance(value, numbers.Real):
            raise VerificationError("matcher returned a nonnumeric score")
        score = float(value)
        if not math.isfinite(score) or not 0.0 <= score <= 1.0:
            raise VerificationError("matcher returned an invalid score")
        identity = (source_names[pair.source_column], target_names[pair.target_column])
        if identity in seen:
            raise VerificationError("matcher returned a duplicate score")
        seen.add(identity)
        scores.append({"source": identity[0], "target": identity[1], "score": score})
    return sorted(scores, key=lambda score: (score["source"], score["target"]))


@contextlib.contextmanager
def offline_guard():
    """Block ordinary Python network entry points; not a security sandbox."""
    def denied(*args, **kwargs):
        raise VerificationError("network access and automatic resource downloads are disabled")
    patches = [(socket, "create_connection"), (socket.socket, "connect"), (socket.socket, "connect_ex"),
               (urllib.request, "urlopen"), (urllib.request, "urlretrieve")]
    originals = [(obj, name, getattr(obj, name)) for obj, name in patches]
    for obj, name, _ in originals:
        setattr(obj, name, denied)
    try:
        yield denied
    finally:
        for obj, name, value in reversed(originals):
            setattr(obj, name, value)


@contextlib.contextmanager
def quiet():
    # Discard output rather than accumulating potentially value-bearing logs.
    with open(os.devnull, "w", encoding="utf8") as sink, contextlib.redirect_stdout(sink), contextlib.redirect_stderr(sink):
        yield


def score_modes(tables, pairs, nltk_data, before, prepared_hash, output):
    with offline_guard() as denied, quiet():
        import nltk
        nltk.data.path[:] = [str(checked(nltk_data))]
        nltk.download = denied
        nltk.downloader.Downloader.download = denied
        # Validate the local resource before importing matchers or using samples.
        from nltk.corpus import stopwords
        stopwords.words("english")
        import pandas as pd
        from valentine import valentine_match
        from valentine.algorithms import Coma
        import valentine.utils.utils as utils
        import valentine.algorithms.coma.similarity.tfidf as tfidf
        utils.ensure_nltk_data = denied
        tfidf.ensure_nltk_data = denied
        tfidf._english_stopwords.cache_clear()
        tfidf._english_stopwords()
        for mode in MODES:
            results = []
            for pair in pairs:
                source, target = tables[pair["source_table"]], tables[pair["target_table"]]
                frames = [pd.DataFrame({name: pd.Series(values, dtype=object)
                                        for name, values in columns(table, mode).items()})
                          for table in (source, target)]
                matcher = Coma(max_n=0, delta=1.0, threshold=0.0, use_schema=True,
                               use_instances=mode == "schema_and_samples", instance_weight=1.0)
                matches = valentine_match(frames, matcher, df_names=["aaa", "bbb"], instance_sample_size=None)
                results.append({"id": pair["id"], "scores": translate_scores(matches, source, target)})
            artifact = {"schema_version": 1,
                        "corpus_sha256": before["input_sha256"][INPUTS[0]],
                        "protocol_sha256": before["input_sha256"][INPUTS[1]],
                        "producer": {"name": "valentine", "version": "1.0.0", "mode": mode,
                                     "provenance": {"prepare_sha256": prepared_hash, "context_sha256": canonical_hash(before),
                                                    "python": sys.version, "settings": SETTINGS,
                                                    "use_instances": mode == "schema_and_samples",
                                                    "schema_values": "64 nulls" if mode == "schema_only" else "fixture strings/nulls padded to 64"}},
                        "tables": results}
            write_new(output / ARTIFACTS[mode], artifact)


def prepare(output, nltk_data):
    if output.exists():
        raise VerificationError("use a fresh comparison output directory")
    before = context(nltk_data)
    comparison_inputs(load(ROOT / INPUTS[0]))
    if context(nltk_data) != before:
        raise VerificationError("comparison inputs changed during preparation")
    output.mkdir(parents=True, exist_ok=False)
    write_new(output / "prepare.json", {"protocol": PROTOCOL, "settings": SETTINGS, "modes": list(MODES),
              "context": before, "context_sha256": canonical_hash(before),
              "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "trust_boundary": "Local drift guard; not a sandbox, signed record or hermetic attestation."})


def run(output, nltk_data):
    if set(path.name for path in output.iterdir()) != {"prepare.json"}:
        raise VerificationError("run requires a prepared directory with no prior or partial results")
    record = load(output / "prepare.json")
    before = context(nltk_data)
    if record["protocol"] != PROTOCOL or record["settings"] != SETTINGS or record["modes"] != list(MODES) or record["context"] != before or record["context_sha256"] != canonical_hash(before):
        raise VerificationError("prepared context differs from current inputs or environment")
    prepared_hash = digest(output / "prepare.json")
    tables, pairs = comparison_inputs(load(ROOT / INPUTS[0]))
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    score_modes(tables, pairs, nltk_data, before, prepared_hash, output)
    after = context(nltk_data)
    if after != before or digest(output / "prepare.json") != prepared_hash:
        raise VerificationError("inputs or environment changed while scoring; no success record written")
    artifacts = {name: digest(output / name) for name in ARTIFACTS.values()}
    write_new(output / "run.json", {"protocol": PROTOCOL, "status": "complete", "pairs_per_mode": len(pairs),
              "modes": list(MODES), "prepare_sha256": prepared_hash, "context_sha256": canonical_hash(after),
              "artifacts_sha256": artifacts, "started_utc": started,
              "completed_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["prepare", "run"])
    parser.add_argument("--output", required=True, type=pathlib.Path)
    parser.add_argument("--nltk-data", required=True, type=pathlib.Path)
    args = parser.parse_args()
    try:
        output, nltk_data = checked(args.output), checked(args.nltk_data)
        if output == ROOT or not output.is_relative_to(ROOT):
            raise VerificationError("output must stay under the workspace")
        (prepare if args.action == "prepare" else run)(output, nltk_data)
    except Exception:
        # Do not print third-party exceptions, tracebacks, frames or sample data.
        print("Comparison failed verification or execution; no success record was written. Retain any partial output for inspection.", file=sys.stderr)
        return 1
    print(f"Valentine comparison {args.action} completed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
