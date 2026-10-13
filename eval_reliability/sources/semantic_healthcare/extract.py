#!/usr/bin/env python3
"""Extract publisher schemas and context only; never reads cases or answers."""
from __future__ import annotations
import argparse, hashlib, json, pathlib, re, subprocess, tempfile, urllib.parse, urllib.request

PUBLISHER = "OHDSI/CommonDataModel"
SOURCES = [
    {"id": "source_ddl", "tag": "v5.0.1", "commit": "2ab4be78c4c57100b93005b1689617b6c62d4ffd", "path": "PostgreSQL/OMOP CDM ddl - PostgreSQL.sql", "sha256": "648514ea0dcbbc47817d0b05a4816abb2ae93e84db4c81b2d44617ef66979c5e"},
    {"id": "source_pdf", "tag": "v5.0.1", "commit": "2ab4be78c4c57100b93005b1689617b6c62d4ffd", "path": "OMOP CDM v5.pdf", "sha256": "d0ac7e9e499ea947068708471225f631c31ba0c72478acd0e306472b5b2e7977"},
    {"id": "source_repo_license", "tag": "v5.0.1", "commit": "2ab4be78c4c57100b93005b1689617b6c62d4ffd", "path": "LICENSE", "sha256": "cb5e8e7e5f4a3988e1063c142c60dc2df75605f4c46515e776e3aca6df976e14"},
    {"id": "target_ddl", "tag": "v5.2.2", "commit": "3f49979dd407c67fe452e4b308464ce05d0a577c", "path": "PostgreSQL/OMOP CDM ddl - PostgreSQL.sql", "sha256": "886e1adfe021b537e7f0adf8b5bb222ada98ea825d6cdfeffca718a813a5dbe6"},
    {"id": "target_cost_doc", "tag": "v5.2.2", "commit": "3f49979dd407c67fe452e4b308464ce05d0a577c", "path": "Documentation/CommonDataModel_Wiki_Files/StandardizedHealthEconomicsDataTables/COST.md", "sha256": "de7fa61b0973366650f7b05b4c4c3617c07b7b2f567a1ca6ebc8002736b09a54"},
    {"id": "target_doc_license", "tag": "v5.2.2", "commit": "3f49979dd407c67fe452e4b308464ce05d0a577c", "path": "Documentation/CommonDataModel_Wiki_Files/License.md", "sha256": "47916d758df3abf2a5a80d76dddf8632711a34cc2865c8edf0b25116790a1c4a"},
]

def dump(path, obj):
    path.write_text(json.dumps(obj, ensure_ascii=False, indent=2) + "\n")

def sql_table(text, table):
    match = re.search(r"CREATE TABLE\s+" + table + r"\s*\((.*?)\)\s*;", text, re.I | re.S)
    assert match, table
    rows = []
    for line in match.group(1).splitlines():
        m = re.match(r"\s*(\w+)\s+(INTEGER|NUMERIC|VARCHAR\s*\(\d+\))\s+(NOT NULL|NULL)\s*,?\s*$", line, re.I)
        if m:
            name, dtype, nullable = m.groups()
            declared = re.sub(r"\s+", "", dtype).upper()
            enum = "integer" if declared == "INTEGER" else "decimal" if declared == "NUMERIC" else "text"
            rows.append({"name": name, "declared_type": declared, "data_type": enum, "required": nullable.upper() == "NOT NULL"})
        else:
            assert not line.strip(), line
    return rows, match.group(0) + "\n"

def procedure_pdf(text):
    start = text.index("7.3 PROCEDURE_COST\n")
    end = text.index("7.4 DRUG_COST\n", start)
    excerpt = text[start:end]
    definition, rest = excerpt.split(" Field", 1)
    table, conventions = rest.split("7.3.1 CONVENTIONS", 1)
    rows = []
    for line in table.splitlines():
        m = re.match(r"\s*(\w+)\s+(Yes|No)\s+(integer|float|varchar\(\d+\))\s+(.+)$", line)
        if m:
            name, required, dtype, description = m.groups()
            rows.append({"documented_name": name, "documented_type": dtype, "description": description.strip(), "documented_required": required})
        elif rows and len(line) - len(line.lstrip()) >= 45 and line.strip() and not line.strip().isdigit():
            rows[-1]["description"] += " " + line.strip()
    return rows, definition.split("\n", 1)[1].strip(), conventions.strip(), excerpt

def cost_markdown(text):
    rows = []
    for line in text.splitlines():
        if not line.startswith("|"):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) != 4:
            continue
        name, required, dtype, description = cells
        if required not in ("Yes", "No"):
            # The final two publisher rows transpose Type and Required.
            assert dtype in ("Yes", "No"), cells
            required, dtype = dtype, required
        rows.append({"documented_name": name, "documented_type": dtype, "documented_required": required, "description": description})
    definition = text.split("\n\nField|", 1)[0]
    conventions = text.split("### Conventions", 1)[1].strip()
    return rows, definition, conventions

def link_context(table, physical, documented, definition, conventions, version, ddl_id, doc_id):
    assert len(physical) == len(documented)
    fields = []
    for p, d in zip(physical, documented):
        if p["name"] != d["documented_name"]:
            # Same-side discrepancy between two official target declarations.
            assert (p["name"], d["documented_name"]) == ("reveue_code_source_value", "revenue_code_source_value")
        fields.append({"name": f"{table}.{p['name']}", "table": table, "physical_field_name": p["name"], "documented_field_name": d["documented_name"], "declared_type": p["declared_type"], "required": p["required"], **{k: v for k, v in d.items() if k != "documented_name"}, "declaration_source": ddl_id, "description_source": doc_id})
    return {"schema_version": version, "table": table, "table_description": definition, "table_conventions": conventions, "description_source": doc_id, "fields": fields}

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", type=pathlib.Path, default=pathlib.Path(__file__).resolve().parent)
    args = parser.parse_args()
    out = args.output_dir
    raw = out / "raw"
    raw.mkdir(parents=True, exist_ok=True)
    data = {}
    provenance = []
    for item in SOURCES:
        url = "https://raw.githubusercontent.com/" + PUBLISHER + "/" + item["commit"] + "/" + urllib.parse.quote(item["path"])
        content = urllib.request.urlopen(url).read()
        assert hashlib.sha256(content).hexdigest() == item["sha256"], item["id"]
        data[item["id"]] = content
        provenance.append({**item, "url": url, "full_source_bytes": len(content)})
    with tempfile.TemporaryDirectory(prefix="fieldkin-ohdsi-source-") as tmp:
        pdf = pathlib.Path(tmp) / "schema.pdf"
        pdf.write_bytes(data["source_pdf"])
        text = subprocess.check_output(["pdftotext", "-layout", str(pdf), "-"], text=True)
    sp, sexcerpt = sql_table(data["source_ddl"].decode(), "procedure_cost")
    tp, texcerpt = sql_table(data["target_ddl"].decode(), "cost")
    sd, sdefinition, sconventions, pexcerpt = procedure_pdf(text)
    td, tdefinition, tconventions = cost_markdown(data["target_cost_doc"].decode())
    assert len(sp) == len(sd) == 13
    assert len(tp) == len(td) == 22
    source_context = link_context("procedure_cost", sp, sd, sdefinition, sconventions, "5.0.1", "source_ddl", "source_pdf")
    target_context = link_context("cost", tp, td, tdefinition, tconventions, "5.2.2", "target_ddl", "target_cost_doc")
    schemas = {}
    for side, table, rows in [("source", "procedure_cost", sp), ("target", "cost", tp)]:
        schemas[side] = {"fields": [{"name": f"{table}.{row['name']}", "data_type": row["data_type"], "samples": []} for row in rows]}
    dump(out / "schemas.json", schemas)
    dump(out / "contexts.json", {"source": source_context, "target": target_context})
    dump(out / "provenance.json", {"publisher": PUBLISHER, "publisher_url": "https://github.com/OHDSI/CommonDataModel", "retrieved_utc": "2026-10-12", "schema_only": True, "samples_policy": "No attributable field-level observations were present in the selected schema sources; all sample arrays are empty.", "naming": "Case names preserve PostgreSQL DDL table and field identifiers. PDF/Markdown logical identifiers are retained separately in context. Documentation and physical schema monetary types are float and NUMERIC respectively; NUMERIC is normalized to decimal.", "source_documents": provenance, "license_evidence": {"source_repository": "Apache-2.0 (source_repo_license)", "source_pdf": "Public-domain dedication / CC0-1.0 declaration on PDF page 2", "target_documentation": "Public-domain dedication / CC0-1.0 declaration (target_doc_license)"}, "known_publisher_discrepancies": ["Target PostgreSQL field reveue_code_source_value is spelled revenue_code_source_value in the official logical schema; both are retained.", "The target Markdown DRG rows transpose Required and Type; extraction identifies the Yes/No cell and corroborates types using PostgreSQL DDL.", "Source copay prose excludes copay from out-of-pocket but procedure conventions include it; the total field and aggregate formula are used for intended schema correspondence.", "Target patient payment conventions distinguish claim responsibility from verified actual payments; this evaluation concerns intended field roles, not universally lossless ETL."]})
    (raw / "source_procedure_cost_ddl.sql").write_text(sexcerpt)
    (raw / "target_cost_ddl.sql").write_text(texcerpt)
    (raw / "source_procedure_cost_pdf_excerpt.txt").write_text(pexcerpt)
    (raw / "source_pdf_license_excerpt.txt").write_text(text.split("\f")[1])
    (raw / "source_repository_LICENSE.txt").write_bytes(data["source_repo_license"])
    (raw / "target_cost_documentation.md").write_bytes(data["target_cost_doc"])
    (raw / "target_documentation_license.md").write_bytes(data["target_doc_license"])
    print("Extracted 13 source fields and 22 target fields; no answers read or produced.")

if __name__ == "__main__":
    main()
