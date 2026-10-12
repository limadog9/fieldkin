#!/usr/bin/env python3
"""Offline, source-only USAspending subaward crosswalk extraction; stdlib only.

Writes financial_cases.frozen.json and evidence.json beside this script.
Never executes publisher code, accesses a matcher, or constructs sample values.
"""
import ast
import hashlib
import json
from pathlib import Path
import re
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parent
CASE = "treasury_contract_subaward_download_to_broker_schema_only"
SOURCE_FILE = "Contracts_Subawards.csv"
TARGET_TABLE = "broker_subaward"
SOURCE_ROWS = [298, 301, 303, 350, 351, 352, 354, 356, 358, 360, 362, 363,
               364, 365, 366, 367, 368, 369, 370, 371, 380, 381, 382, 383,
               386, 388, 389]
NO_MATCH = {
    298: "Prime-recipient award amount is not subrecipient award amount. The target projection omits award_amount and retains subaward_amount; the financial recipient role differs.",
    303: "Prime-contractor business classifications are not the sub-contractor's business classifications. The prime business_types counterpart is omitted while sub_business_types remains.",
    354: "The prime awardee's possibly nonunique tracking number is not the FSRS report ID. Crosswalk explicitly states one report may contain multiple subaward records; subaward_number is omitted.",
    370: "An executive's preceding-fiscal-year cash and noncash earnings are not an amount awarded to a subrecipient. The compensation-amount counterpart is omitted. No competing numbered officer amount is included.",
    380: "Subrecipient legal-business street address is not the subaward performance street address or the prime recipient legal address. The sub_legal_entity_address_line1 counterpart is omitted.",
    388: "Subrecipient legal-business ZIP code is not the place-of-performance ZIP code or the prime recipient legal-business ZIP code. The sub_legal_entity_zip counterpart is omitted.",
}
TARGET_ONLY = [35, 205, 208, 217, 220, 355]
NS = {"s": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}


def dump(name, value):
    (ROOT / name).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def parts(value):
    return [p.strip() for p in value.split(",")]


def main():
    resources = json.loads((ROOT / "resources.json").read_text())
    for r in resources:
        assert hashlib.sha256((ROOT / r["path"]).read_bytes()).hexdigest() == r["sha256"], r["path"]
    with zipfile.ZipFile(ROOT / "raw/Data_Dictionary_Crosswalk.xlsx") as z:
        strings = ["".join(e.itertext()) for e in ET.fromstring(z.read("xl/sharedStrings.xml")).findall("s:si", NS)]
        rows = {}
        for row in ET.fromstring(z.read("xl/worksheets/sheet4.xml")).findall("s:sheetData/s:row", NS):
            cells = {}
            for cell in row.findall("s:c", NS):
                node = cell.find("s:v", NS)
                value = "" if node is None else node.text
                if cell.get("t") == "s":
                    value = strings[int(value)]
                cells[re.sub(r"\d", "", cell.get("r"))] = value
            rows[int(row.get("r"))] = cells
    text = (ROOT / "raw/broker_subawards.py").read_text()
    assignments = [n for n in ast.parse(text).body if isinstance(n, ast.Assign)
                   and any(isinstance(t, ast.Name) and t.id == "BROKER_SUBAWARDS_COLUMNS" for t in n.targets)]
    assert len(assignments) == 1
    declaration = assignments[0].value
    column_types = ast.literal_eval(declaration)
    lines = {ast.literal_eval(k): k.lineno for k in declaration.keys}
    type_map = {"TEXT": "text", "INTEGER": "integer", "TIMESTAMP": "timestamp", "LONG NOT NULL": "integer"}

    def names(rownum):
        row = rows[rownum]
        assert SOURCE_FILE in parts(row["I"]), (rownum, row["I"])
        source = row["J"]
        assert source and source != "N/A" and "," not in source, (rownum, source)
        tables, fields = parts(row["M"]), parts(row["N"])
        assert TARGET_TABLE in tables, (rownum, tables)
        target = fields[0] if len(fields) == 1 else fields[tables.index(TARGET_TABLE)]
        assert len(fields) in [1, len(tables)], (rownum, tables, fields)
        assert target in column_types, (rownum, target)
        return source, target

    target_rows = sorted([r for r in SOURCE_ROWS if r not in NO_MATCH] + TARGET_ONLY)
    source = [{"name": names(r)[0], "data_type": "unknown", "samples": []} for r in SOURCE_ROWS]
    target = [{"name": names(r)[1], "data_type": type_map[column_types[names(r)[1]]["postgres"]], "samples": []} for r in target_rows]
    answers = [{"kind": "no_match", "source": names(r)[0]} if r in NO_MATCH else
               {"kind": "match", "source": names(r)[0], "target": names(r)[1]} for r in SOURCE_ROWS]
    assert len({f["name"] for f in source}) == len(source) == len(answers)
    assert len({f["name"] for f in target}) == len(target)
    assert {a["source"] for a in answers} == {f["name"] for f in source}
    assert all(a.get("target") in {f["name"] for f in target} for a in answers if a["kind"] == "match")
    case = {"name": CASE, "source": {"fields": source}, "target": {"fields": target}, "answers": answers}
    evidence = []
    for r in sorted(set(SOURCE_ROWS + target_rows)):
        src, dst = names(r)
        evidence.append({
            "crosswalk_sheet": "Public", "crosswalk_row": r, "concept": rows[r]["A"], "definition": rows[r]["B"],
            "source_csv_file": SOURCE_FILE, "source_field": src, "source_storage_type": "unknown: publisher crosswalk declares no CSV storage type",
            "target_table": TARGET_TABLE, "target_field": dst, "target_declared_types": column_types[dst],
            "target_normalized_type": type_map[column_types[dst]["postgres"]], "type_source": "raw/broker_subawards.py",
            "type_declaration_line": lines[dst], "type_dictionary": "BROKER_SUBAWARDS_COLUMNS",
            "publisher_mapping_cells": {c: rows[r][c] for c in ["I", "J", "M", "N"]},
            "source_in_projection": r in SOURCE_ROWS, "target_in_projection": r in target_rows,
            "answer": "no_match" if r in NO_MATCH else "match" if r in SOURCE_ROWS else "target_only",
            "reason": NO_MATCH.get(r, "The same publisher crosswalk concept row explicitly maps the exact subaward CSV name to the selected broker_subaward database name."),
        })
    dump("financial_cases.frozen.json", [case])
    dump("evidence.json", {"case": CASE, "source_rows": SOURCE_ROWS, "target_rows": target_rows,
                           "source_fields": [f["name"] for f in source], "target_fields": [f["name"] for f in target],
                           "field_evidence": evidence})
    print(f"Generated one case: {len(source)} source fields, {len(target)} target fields; all labels present; no predictions.")


if __name__ == "__main__":
    main()
