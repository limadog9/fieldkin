#!/usr/bin/env python3
"""Deterministic stdlib extraction of Treasury publisher schema projections.

Run beside archived Data_Dictionary_Crosswalk.xlsx and publisher model .py files.
No network calls, imports of publisher code, matcher access, or sample invention.
"""
import ast
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parent
SHA = "03b9e2554837998c4c261c65e2947dc79f23855a"
BASE = f"https://raw.githubusercontent.com/fedspendingtransparency/usaspending-api/{SHA}/"
ACQUISITION_TIMES = {'Data_Dictionary_Crosswalk.xlsx': '2026-10-12T19:06:13.364824+00:00', 'transaction_fabs.py': '2026-10-12T19:07:02.709359+00:00', 'transaction_fpds.py': '2026-10-12T19:07:02.689179+00:00', 'custom_django_fields.py': '2026-10-12T19:08:05.523750+00:00', 'LICENSE': '2026-10-12T19:05:49.990345+00:00'}
NS = {"s": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}
with zipfile.ZipFile(ROOT / "Data_Dictionary_Crosswalk.xlsx") as z:
    strings = ["".join(e.itertext()) for e in ET.fromstring(z.read("xl/sharedStrings.xml")).findall("s:si", NS)]
    rows = {}
    for row in ET.fromstring(z.read("xl/worksheets/sheet4.xml")).findall("s:sheetData/s:row", NS):
        cells = {}
        for c in row.findall("s:c", NS):
            v = c.find("s:v", NS)
            value = "" if v is None else v.text
            if c.get("t") == "s":
                value = strings[int(value)]
            cells[re.sub(r"\d", "", c.get("r"))] = value
        rows[int(row.get("r"))] = cells

TYPE_MAP = {"TextField": "text", "CharField": "text", "IntegerField": "integer", "BigIntegerField": "integer", "DecimalField": "decimal", "NumericField": "decimal", "FloatField": "float", "BooleanField": "boolean", "DateField": "date", "DateTimeField": "timestamp"}

def model_fields(model):
    text = (ROOT / (model + ".py")).read_text()
    result = {}
    for obj in ast.parse(text).body:
        if not isinstance(obj, ast.ClassDef):
            continue
        for a in obj.body:
            if not isinstance(a, ast.Assign) or not isinstance(a.value, ast.Call):
                continue
            func = a.value.func
            declared = func.attr if isinstance(func, ast.Attribute) else func.id if isinstance(func, ast.Name) else "unknown"
            for target in a.targets:
                if isinstance(target, ast.Name):
                    result[target.id] = {"type": TYPE_MAP.get(declared, "unknown"), "declared": declared, "line": a.lineno, "declaration": ast.get_source_segment(text, a)}
    return result

SPECS = [
    {"name": "treasury_assistance_download_to_database_schema_only", "model": "transaction_fabs", "file": "Assistance_PrimeTransactions.csv", "source_rows": [26,27,28,52,53,54,55,58,61,89,90,134,154,155,189,207,214,216,246,263,287,293,294,313,314,327,142], "omit": [28,134,214,314,142], "target_only": [212,37,150,38,151], "reasons": {28: "Assistance type description is distinct from its code; description counterpart omitted.", 134: "Loan face value is distinct from net-present-value loan subsidy cost; face-value counterpart omitted.", 214: "Recipient legal-address foreign city differs from foreign place of performance; city counterpart omitted.", 314: "Record-type description is distinct from record-type code; description counterpart omitted.", 142: "Federal obligation differs from nonfederal funding amount and loan subsidy cost; federal obligation counterpart omitted."}},
    {"name": "treasury_contract_download_to_database_schema_only", "model": "transaction_fpds", "file": "Contracts_PrimeTransactions.csv", "source_rows": [43,44,99,284,261,278,345,93,94,129,130,131,132,249,305,306,342,343,344,413,414,417,418,100,101], "omit": [43,261,94,130,344], "target_only": [91,92,96,97,285], "reasons": {43: "Transaction change to potential contract value is distinct from cumulative potential award value; transaction-delta counterpart omitted.", 261: "Ordering-period end date differs from potential performance end date and solicitation date; ordering counterpart omitted.", 94: "Cost/pricing-data description is distinct from cost/pricing-data code; description counterpart omitted.", 130: "Evaluated preference description is distinct from its code; description counterpart omitted.", 344: "Solicitation-procedures description differs from procedures code and solicitation identifier; description counterpart omitted."}},
]

cases, evidence = [], []
for spec in SPECS:
    declared = model_fields(spec["model"])
    target_rows = sorted([r for r in spec["source_rows"] if r not in spec["omit"]] + spec["target_only"])
    def names(r):
        row = rows[r]
        assert spec["file"] in row["G"], (r, row["G"])
        assert spec["model"] in [s.strip() for s in row["M"].split(",")], (r, row["M"])
        assert "," not in row["H"] and "," not in row["N"], (r, row)
        assert row["N"] in declared, (r, row["N"])
        return row["H"], row["N"]
    source = [{"name": names(r)[0], "data_type": "unknown", "samples": []} for r in spec["source_rows"]]
    target = [{"name": names(r)[1], "data_type": declared[names(r)[1]]["type"], "samples": []} for r in target_rows]
    answers = [{"kind": "no_match", "source": names(r)[0]} if r in spec["omit"] else {"kind": "match", "source": names(r)[0], "target": names(r)[1]} for r in spec["source_rows"]]
    assert len({f["name"] for f in source}) == len(source)
    assert len({f["name"] for f in target}) == len(target)
    case = {"name": spec["name"], "source": {"fields": source}, "target": {"fields": target}, "answers": answers}
    cases.append(case)
    for r in sorted(set(spec["source_rows"] + target_rows)):
        s, t = names(r)
        evidence.append({"case": spec["name"], "crosswalk_sheet": "Public", "crosswalk_row": r, "concept": rows[r]["A"], "definition": rows[r]["B"], "source_file": spec["file"], "source_field": s, "target_model": spec["model"], "target_field": t, "target_declared_type": declared[t], "source_type": "unknown; CSV crosswalk declares no storage type", "included_source": r in spec["source_rows"], "included_target": r in target_rows, "answer": "no_match" if r in spec["omit"] else "match" if r in spec["source_rows"] else "target_only", "reason": spec["reasons"].get(r, "Publisher crosswalk maps this named concept to both exact fields.")})

def save(name, obj):
    (ROOT / name).write_text(json.dumps(obj, indent=2, ensure_ascii=False) + "\n")
save("treasury_cases.frozen.json", cases)
save("treasury_evidence.frozen.json", evidence)
sources = {
    "Data_Dictionary_Crosswalk.xlsx": "usaspending_api/data/Data_Dictionary_Crosswalk.xlsx",
    "transaction_fabs.py": "usaspending_api/awards/models/transaction_fabs.py",
    "transaction_fpds.py": "usaspending_api/awards/models/transaction_fpds.py",
    "custom_django_fields.py": "usaspending_api/common/custom_django_fields.py",
    "LICENSE": "LICENSE",
}
save("treasury_manifest.frozen.json", {
    "publisher": "United States Department of the Treasury, USAspending", "repository": "https://github.com/fedspendingtransparency/usaspending-api", "commit": SHA, "retrieval_date_utc": "2026-10-12", "frozen_at_utc": "2026-10-12T19:10:45.798789+00:00", "license": "CC0-1.0", "schema_only": True,
    "record_provenance": "No observed record values used. Empty samples intentionally measure missing-value-evidence behavior. No test/evaluation fixtures accessed.",
    "selection": "Crosswalk row projections selected solely from publisher definitions, financial quantity distinctions, code-versus-description distinctions and entity/event roles before any matcher predictions. Exact field names retained. Target ordering follows publisher crosswalk row order.",
    "type_policy": "Source CSV types are unknown because crosswalk does not declare storage types. Target types preserve publisher Django declarations mapped to evaluation vocabulary; NumericField is decimal because publisher custom_django_fields.py declares PostgreSQL numeric. No type inferred from names or empty samples.",
    "crosswalk_authority": "DEC Overview!A4 explicitly says the document specifies how data elements are labelled across environments; DEC Overview!A7 says it is generated from DAIMS. Public columns G/H are download file/field; M/N are database table/field. Definition B supplies role/unit/event semantics.",
    "blind_review_revision": "Before predictions, independent annotation review replaced target-only row 208 LegalEntityCityName with row 212 LegalEntityCountyCode because generic legal-city wording could encompass foreign cities. No source fields or labels changed. Acquisition timestamps are recorded constants for portable deterministic regeneration.",
    "historical_note": "Crosswalk is the published file at pinned commit and describes Database Download names. Current model files declare compatibility-view fields; this corpus claims equivalence only for exact selected crosswalk rows validated against declared model fields, not blanket equivalence of current storage schemas.",
    "sources": [{"local_file": name, "url": BASE + path, "sha256": hashlib.sha256((ROOT/name).read_bytes()).hexdigest(), "bytes": (ROOT/name).stat().st_size, "retrieved_at_utc": ACQUISITION_TIMES[name]} for name,path in sources.items()],
    "artifacts": [{"local_file": name, "sha256": hashlib.sha256((ROOT/name).read_bytes()).hexdigest()} for name in ["extract_cases.py", "treasury_cases.frozen.json", "treasury_evidence.frozen.json"]],
    "reproduction": "python extract_cases.py (Python 3 standard library only; offline; does not import or execute publisher code)",
    "cases": [{"name": c["name"], "source_fields": len(c["source"]["fields"]), "target_fields": len(c["target"]["fields"]), "match_labels": sum(a["kind"]=="match" for a in c["answers"]), "no_match_labels": sum(a["kind"]=="no_match" for a in c["answers"])} for c in cases],
})
print(json.dumps({"cases": len(cases), "labels": sum(len(c["answers"]) for c in cases)}, indent=2))
