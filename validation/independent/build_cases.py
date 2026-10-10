#!/usr/bin/env python3
"""One-time extraction of the frozen corpus; not needed to run the Rust benchmark.

Requires Python 3, pandas and rdata 1.1.0 only for the publisher .rda files.
Default: verify the committed extraction. --write: intentionally replace cases
and their fingerprints from the already annotated manifest. Never reads matcher
predictions. Publisher snapshots are always verified before extraction.
"""
import argparse
import csv
import datetime
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "validation/independent_corpus.json"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def extract_view(view, resources):
    path = ROOT / resources[view["resource"]]["path"]
    names = [field["name"] for field in view["fields"]]
    if view["format"] == "csv":
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            assert [n for n in reader.fieldnames if n not in view["excluded_fields"]] == names
            rows = list(reader)
        get = lambda row, name: "" if row[name] in ("", "NA") else row[name]
    else:
        import warnings
        import pandas as pd
        import rdata

        with warnings.catch_warnings():
            warnings.simplefilter("ignore", UserWarning)
            table = rdata.read_rda(path)[view["object"]]
        assert list(table.columns) == names
        rows = list(table.itertuples(index=False, name=None))
        types = {field["name"]: field["data_type"] for field in view["fields"]}
        positions = {name: i for i, name in enumerate(names)}

        def get(row, name):
            value = row[positions[name]]
            if pd.isna(value):
                return ""
            if types[name] == "timestamp":
                return datetime.datetime.fromtimestamp(float(value), datetime.timezone.utc).isoformat()
            if types[name] == "integer":
                return str(int(value))
            if types[name] == "float":
                return repr(float(value))
            return str(value)

    assert len(rows) == view["rows"] and rows, "changed/empty publisher table"
    count = min(16, len(rows))
    indices = [i * (len(rows) - 1) // (count - 1) for i in range(count)] if count > 1 else [0]
    return {"fields": [{**field, "samples": [get(rows[i], field["name"]) for i in indices]}
                       for field in view["fields"]]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    manifest = json.loads(MANIFEST.read_text())
    for key, resource in manifest["resources"].items():
        assert digest((ROOT / resource["path"]).read_bytes()) == resource["sha256"], key
    views = {key: extract_view(view, manifest["resources"]) for key, view in manifest["views"].items()}
    for identifier, metadata in manifest["datasets"].items():
        source = views[metadata["source_view"]]
        answers = []
        assert set(metadata["evidence"]) == {f["name"] for f in source["fields"]}
        for field in source["fields"]:
            label = metadata["evidence"][field["name"]]
            answer = {"kind": label["decision"], "source": field["name"]}
            if label["decision"] == "match":
                answer["target"] = label["target"]
            elif label["decision"] == "ambiguous":
                answer["targets"] = label["targets"]
            else:
                assert label["decision"] == "no_match" and label["target"] is None
            answers.append(answer)
        case = {"name": metadata["name"], "source": source,
                "target": views[metadata["target_view"]], "answers": answers}
        canonical = json.dumps(case, ensure_ascii=False, separators=(",", ":")).encode()
        path = ROOT / identifier
        if args.write:
            path.write_text(json.dumps(case, ensure_ascii=False, indent=2) + "\n")
            metadata["sha256"] = digest(canonical)
        else:
            assert json.loads(path.read_text()) == case, identifier + ": changed extraction"
            assert metadata["sha256"] == digest(canonical), identifier + ": changed fingerprint"
    if args.write:
        MANIFEST.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
    print(f"Verified {len(manifest['datasets'])} cases from {len(views)} publisher views; no predictions run.")


if __name__ == "__main__":
    main()
