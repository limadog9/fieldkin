"""Reproduce the annotation-only T2D fixture offline, without scoring a matcher.

Only the explicitly Apache-licensed correspondence metadata is imported. Raw web
tables and DBpedia values have separate terms and are deliberately not inputs.
"""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import re
import tarfile
import urllib.request

ROOT = Path(__file__).resolve().parent
DIRECTORY = ROOT / "external" / "t2d-v1"
SOURCES = {
    "attributes_complete.tar.gz": {
        "url": "https://webdatacommons.org/webtables/attributes_complete.tar.gz",
        "bytes": 84557,
        "sha256": "c35dbeadab8908a72c3f58315a6710039b292a42a7e765ff4a3e4191f8255ba2",
    },
    "classes_complete.csv": {
        "url": "https://webdatacommons.org/webtables/classes_complete.csv",
        "bytes": 154844,
        "sha256": "6a3724e4cce88e3b10ceb97bbdff6da3c397e7a4f9f1bd24b6ff6a7e47f6956c",
    },
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def partition(class_uri):
    return "holdout" if hashlib.sha256(class_uri.encode()).digest()[0] % 5 == 0 else "development"


def rows(data):
    return list(csv.reader(io.StringIO(data.decode("utf-8"), newline="")))


def parse_classes(data):
    result = {}
    for row in rows(data):
        if len(row) != 4 or not row[0].endswith(".tar.gz") or not row[2]:
            raise ValueError("malformed class annotation")
        table_id = row[0].removesuffix(".tar.gz")
        if table_id in result:
            raise ValueError("duplicate class annotation")
        result[table_id] = {"name": row[1], "uri": row[2]}
    return result


def parse_attributes(data):
    fields = {}
    for row in rows(data):
        if len(row) != 4 or row[2] not in ("True", "False", "") or not row[3].isdigit() or not row[0]:
            raise ValueError("malformed attribute annotation")
        index = int(row[3])
        if index in fields and fields[index]["name"] != row[1]:
            raise ValueError("inconsistent headers for one source index")
        field = fields.setdefault(index, {"index": index, "name": row[1], "positive_targets": []})
        if row[0] not in field["positive_targets"]:
            field["positive_targets"].append(row[0])
    for field in fields.values():
        field["positive_targets"].sort()
    return [fields[index] for index in sorted(fields)]


def convert(attributes, class_bytes):
    class_map = parse_classes(class_bytes)
    tables, excluded = [], []
    classes = {}
    seen = set()
    with tarfile.open(fileobj=io.BytesIO(attributes), mode="r:gz") as archive:
        for member in archive:
            # Read members in memory, never extract paths or follow links.
            if not member.isfile() or not re.fullmatch(r"[0-9_]+\.csv", member.name):
                raise ValueError("unexpected attribute archive member")
            if member.name in seen or member.size > 100_000:
                raise ValueError("duplicate or oversized attribute archive member")
            seen.add(member.name)
            stream = archive.extractfile(member)
            if stream is None:
                raise ValueError("missing archive member")
            fields = parse_attributes(stream.read())
            table_id = member.name.removesuffix(".csv")
            if not fields:
                excluded.append({"table_id": table_id, "reason": "empty_attribute_annotations"})
                continue
            if table_id not in class_map:
                excluded.append({"table_id": table_id, "reason": "missing_class_annotation"})
                continue
            class_info = class_map[table_id]
            uri = class_info["uri"]
            target = classes.setdefault(uri, {"uri": uri, "name": class_info["name"], "partition": partition(uri), "properties": set()})
            if target["name"] != class_info["name"]:
                raise ValueError("class URI has inconsistent names")
            target["properties"].update(p for field in fields for p in field["positive_targets"])
            tables.append({"id": table_id, "class_uri": uri, "fields": fields})
    class_list = []
    for uri in sorted(classes):
        item = classes[uri]
        item["properties"] = [{"uri": prop, "name": re.split(r"[/#]", prop)[-1]} for prop in sorted(item["properties"])]
        class_list.append(item)
    return {
        "format": "fieldkin-t2d-annotations-v1",
        "classes": class_list,
        "tables": sorted(tables, key=lambda item: item["id"]),
        "excluded": sorted(excluded, key=lambda item: item["table_id"]),
    }


def encoded(value):
    # One table/class per line keeps the independent fixture reviewable.
    lines = ['{', '  "format": ' + json.dumps(value["format"]) + ',']
    for key in ("classes", "tables", "excluded"):
        lines.append(f'  "{key}": [')
        lines.extend("    " + json.dumps(item, ensure_ascii=False, separators=(",", ":")) + ("," if i + 1 < len(value[key]) else "") for i, item in enumerate(value[key]))
        lines.append("  ]" + ("," if key != "excluded" else ""))
    return ("\n".join(lines) + "\n}\n").encode()


def load_sources(download=False):
    result = {}
    for name, source in SOURCES.items():
        path = DIRECTORY / name
        if download:
            with urllib.request.urlopen(source["url"], timeout=30) as response:
                data = response.read(source["bytes"] + 1)
        else:
            data = path.read_bytes()
        if len(data) != source["bytes"] or digest(data) != source["sha256"]:
            raise ValueError(f"upstream snapshot integrity failure: {name}")
        if download:
            if path.exists() and path.read_bytes() != data:
                raise ValueError(f"refusing to replace changed input: {path}")
            path.write_bytes(data)
        result[name] = data
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="compare the committed derived fixture, without rewriting")
    parser.add_argument("--download", action="store_true", help="explicitly fetch the two pinned correspondence files")
    args = parser.parse_args()
    sources = load_sources(args.download)
    value = convert(sources["attributes_complete.tar.gz"], sources["classes_complete.csv"])
    data = encoded(value)
    path = DIRECTORY / "corpus.json"
    if args.check:
        if path.read_bytes().replace(b"\r\n", b"\n") != data:
            raise ValueError("committed annotation fixture differs from its pinned inputs")
    elif path.exists() and path.read_bytes().replace(b"\r\n", b"\n") != data:
        raise ValueError("refusing to replace changed fixture; review source changes first")
    else:
        path.write_bytes(data)
    for name in ("development", "holdout"):
        classes = {item["uri"] for item in value["classes"] if item["partition"] == name}
        tables = [item for item in value["tables"] if item["class_uri"] in classes]
        print(f"{name}: {len(classes)} classes, {len(tables)} tables, {sum(len(t['fields']) for t in tables)} labeled source fields, {sum(len(f['positive_targets']) for t in tables for f in t['fields'])} known-positive edges")
    print(f"corpus SHA256 {digest(data)}; {len(value['excluded'])} excluded annotation files; no matcher executed")


if __name__ == "__main__":
    main()
