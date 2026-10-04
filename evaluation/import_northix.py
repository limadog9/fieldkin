"""Reproduce the licensed Northix fixture without running a matcher.

Inputs come only from InputData. Class directories supply labels, never values,
types, names, or hints. Archive members are read in memory and never extracted.
"""

import argparse
from collections import Counter, defaultdict
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import stat
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent
DIRECTORY = ROOT / "external" / "northix-v1"
FIXTURE = ROOT / "fixtures" / "northix-v1.json"
SOURCE = {
    "url": "https://archive.ics.uci.edu/static/public/237/northix.zip",
    "bytes": 1240631,
    "sha256": "2b158f99e1a041311177e9519c0d93a3821400121b809cbf7cb4614c6635a838",
}
MAX_MEMBERS = 512
MAX_MEMBER_BYTES = 512 * 1024
MAX_EXPANDED_BYTES = 8 * 1024 * 1024
SAMPLE_CAP = 64
FILENAME = re.compile(r"([A-Za-z0-9_]+)@([A-Za-z0-9_]+)@([12])\.(dat|txt)")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode("utf-8")


def sample_indices(count, cap=SAMPLE_CAP):
    """Endpoint-inclusive, evenly spaced integer indices; no randomness or labels."""
    if count < 0 or cap < 1:
        raise ValueError("invalid sample bounds")
    size = min(count, cap)
    if size < 2:
        return list(range(size))
    return [index * (count - 1) // (size - 1) for index in range(size)]


def read_members(data):
    if len(data) > SOURCE["bytes"]:
        raise ValueError("oversized archive")
    result = {}
    expanded = 0
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        members = archive.infolist()
        if len(members) > MAX_MEMBERS:
            raise ValueError("too many archive members")
        seen = set()
        for member in members:
            name = member.filename
            parts = PurePosixPath(name).parts
            if (
                not parts or parts[0] != "Northix" or name.startswith("/") or name != member.orig_filename
                or name.rstrip("/") != PurePosixPath(name).as_posix() or "//" in name
                or "\\" in name or ":" in name or any(p in (".", "..") for p in name.split("/"))
                or name in seen or stat.S_ISLNK(member.external_attr >> 16)
                or member.flag_bits & 1 or member.compress_type not in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED)
            ):
                raise ValueError("unsafe, duplicated, encrypted, or unsupported archive member")
            seen.add(name)
            expanded += member.file_size
            if member.file_size > MAX_MEMBER_BYTES or expanded > MAX_EXPANDED_BYTES:
                raise ValueError("archive expansion budget exceeded")
            if member.is_dir():
                continue
            permitted = name == "Northix/ReadMe.txt" or (
                len(parts) == 3 and parts[1] == "InputData" and FILENAME.fullmatch(parts[-1])
            ) or (
                len(parts) == 4 and parts[1] == "Classes" and FILENAME.fullmatch(parts[-1])
            )
            if not permitted:
                raise ValueError("unexpected archive file")
            with archive.open(member) as stream:
                content = stream.read(MAX_MEMBER_BYTES + 1)
            if len(content) != member.file_size or len(content) > MAX_MEMBER_BYTES:
                raise ValueError("archive member size differs from its declaration")
            result[name] = content
    return result


def convert(data):
    members = read_members(data)
    inputs = {}
    for name, value in members.items():
        if "/InputData/" in name:
            field_id = name.rsplit("/", 1)[-1]
            if field_id in inputs:
                raise ValueError("duplicate input field identifier")
            inputs[field_id] = (name, value)
    if not inputs:
        raise ValueError("archive contains no input columns")
    labels = {}
    discrepancies = []
    for path, content in sorted(members.items()):
        if "/Classes/" not in path:
            continue
        parts = path.split("/")
        field_id = parts[-1]
        if field_id not in inputs or field_id in labels:
            raise ValueError("unknown or multiply labeled input column")
        labels[field_id] = {"field": field_id, "class": parts[-2], "class_path": path, "class_sha256": digest(content)}
        input_path, input_content = inputs[field_id]
        if input_content != content:
            discrepancies.append({"field": field_id, "input_path": input_path, "input_sha256": digest(input_content),
                                  "class_path": path, "class_sha256": digest(content),
                                  "input_bytes": len(input_content), "class_bytes": len(content)})
    if set(inputs) != set(labels):
        raise ValueError("not every input column has exactly one class label")
    grouped = defaultdict(list)
    ascii_files = 0
    for field_id, (path, content) in sorted(inputs.items()):
        match = FILENAME.fullmatch(field_id)
        if match is None:
            raise ValueError("malformed input identifier")
        name, table, database, extension = match.groups()
        if extension != ("dat" if database == "1" else "txt"):
            raise ValueError("database identifier and input extension disagree")
        # The pinned inputs use only ASCII and bytes >= 0xA0. Latin-1 and CP1252
        # therefore agree. Refuse newly ambiguous control bytes instead of guessing.
        if any(0x80 <= value <= 0x9F for value in content) or b"\x00" in content:
            raise ValueError("input encoding audit no longer holds")
        ascii_files += content.isascii()
        lines = content.decode("iso-8859-1").splitlines()
        indices = sample_indices(len(lines))
        samples = [lines[index] if lines[index].strip() else None for index in indices]
        grouped[(database, table)].append({"id": field_id, "name": name, "samples": samples,
            "sample_row_indices": indices, "row_count": len(lines), "input_path": path,
            "input_sha256": digest(content), "input_bytes": len(content),
            "blank_rows": sum(not value.strip() for value in lines)})
    tables = []
    for (database, name), fields in sorted(grouped.items()):
        if len({field["name"] for field in fields}) != len(fields):
            raise ValueError("duplicate display names within a native table")
        tables.append({"id": f"db{database}:{name}", "database": database, "name": name, "fields": fields})
    pairs = [{"id": f"{source['id']}->{target['id']}", "source_table": source["id"], "target_table": target["id"]}
             for source in tables if source["database"] == "1"
             for target in tables if target["database"] == "2"]
    return {"format": "fieldkin-northix-v1", "tables": tables, "pairs": sorted(pairs, key=lambda pair: pair["id"]),
            "labels": [labels[field] for field in sorted(labels)], "discrepancies": sorted(discrepancies, key=lambda d: d["field"]),
            "encoding_audit": {"encoding": "ISO-8859-1", "ascii_files": ascii_files,
                               "non_ascii_files": len(inputs) - ascii_files, "cp1252_equivalent": True}}


def inventory(value):
    classes = {label["field"]: label["class"] for label in value["labels"]}
    tables = {table["id"]: table for table in value["tables"]}
    counts = Counter(table_pairs=len(value["pairs"]), unique_fields=len(classes),
                     label_classes=len(set(classes.values())), value_copy_discrepancies=len(value["discrepancies"]))
    for table in tables.values():
        counts[f"database_{table['database']}_tables"] += 1
        counts[f"database_{table['database']}_fields"] += len(table["fields"])
    for pair in value["pairs"]:
        source, target = tables[pair["source_table"]], tables[pair["target_table"]]
        edges = 0
        for field in source["fields"]:
            label = classes[field["id"]]
            positives = [f for f in target["fields"] if label != "UNCLASSED" and label == classes[f["id"]]]
            counts["source_field_occurrences"] += 1
            counts["positive_field_occurrences"] += bool(positives)
            counts["no_match_field_occurrences"] += not positives
            counts["explicit_unclassed_field_occurrences"] += label == "UNCLASSED"
            edges += len(positives)
        counts["positive_edges"] += edges
        counts["pairs_with_positive_edges"] += edges > 0
        counts["pairs_without_positive_edges"] += edges == 0
    return dict(sorted(counts.items()))


def load_source(download=False):
    path = DIRECTORY / "northix.zip"
    if download:
        with urllib.request.urlopen(SOURCE["url"], timeout=30) as response:
            data = response.read(SOURCE["bytes"] + 1)
    else:
        with path.open("rb") as stream:
            data = stream.read(SOURCE["bytes"] + 1)
    if len(data) != SOURCE["bytes"] or digest(data) != SOURCE["sha256"]:
        raise ValueError("Northix snapshot integrity failure")
    if download:
        DIRECTORY.mkdir(parents=True, exist_ok=True)
        if path.exists() and path.read_bytes() != data:
            raise ValueError("refusing to replace changed archive")
        path.write_bytes(data)
    return data


def write_or_check(path, data, check):
    if path.exists():
        if path.read_bytes() != data:
            raise ValueError(f"{path.name} differs from its pinned inputs")
    elif check:
        raise ValueError(f"missing checked artifact: {path.name}")
    else:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--download", action="store_true", help="fetch the exact licensed upstream archive")
    parser.add_argument("--check", action="store_true", help="verify committed fixture and provenance without rewriting")
    args = parser.parse_args()
    value = convert(load_source(args.download))
    data = encoded(value)
    counts = inventory(value)
    provenance = {"dataset": "Northix", "creator": "Farid Bourennani", "year": 2012,
        "doi": "10.24432/C5M60J", "landing_page": "https://archive.ics.uci.edu/dataset/237/northix",
        "license": "CC-BY-4.0", "license_url": "https://creativecommons.org/licenses/by/4.0/",
        "archive": SOURCE, "derived_fixture": {"path": "evaluation/fixtures/northix-v1.json", "sha256": digest(data)},
        "inventory": counts, "conversion": {"encoding": "ISO-8859-1", "sample_cap": SAMPLE_CAP,
        "sample_indices": "floor(i*(n-1)/(min(64,n)-1)); use [] for n=0, [0] for n=1",
        "missing_values": "Whitespace-only or empty lines become null; preserve every other line exactly.",
        "inputs": "Only InputData/ names and values; Classes/ provides labels, never input evidence.",
        "label_scope": "Same non-UNCLASSED class defines an acceptable correspondence; all other pairs are task-defined negatives. UNCLASSED is separately counted as explicitly unmatched.",
        "limitations": "Historical demonstration databases with injected tuples and coarse manually authored equivalence classes; not production consumer validation. Field line records are independently sampled; reconstructed DataFrames imply no row alignment."}}
    write_or_check(FIXTURE, data, args.check)
    write_or_check(DIRECTORY / "provenance.json", encoded(provenance), args.check)
    print(json.dumps({"inventory": counts, "fixture_sha256": digest(data), "matcher_executed": False}, sort_keys=True))


if __name__ == "__main__":
    main()
