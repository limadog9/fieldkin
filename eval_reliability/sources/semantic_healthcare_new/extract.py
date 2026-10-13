#!/usr/bin/env python3
"""Reproduce official schema/context extraction without reading any answers."""
from __future__ import annotations
import argparse, csv, hashlib, io, json, pathlib, re, urllib.parse, urllib.request

SOURCES = [
    {"id":"source_visit_documentation", "tag":"v5.2.2", "commit":"3f49979dd407c67fe452e4b308464ce05d0a577c", "path":"Documentation/CommonDataModel_Wiki_Files/StandardizedClinicalDataTables/VISIT_OCCURRENCE.md", "sha256":"cef08995bee75ea536be5bb96bf74e9ecc37377408c5a48aa72fd72d96e30c77"},
    {"id":"source_ddl", "tag":"v5.2.2", "commit":"3f49979dd407c67fe452e4b308464ce05d0a577c", "path":"PostgreSQL/OMOP CDM ddl - PostgreSQL.sql", "sha256":"886e1adfe021b537e7f0adf8b5bb222ada98ea825d6cdfeffca718a813a5dbe6"},
    {"id":"source_license", "tag":"v5.2.2", "commit":"3f49979dd407c67fe452e4b308464ce05d0a577c", "path":"Documentation/CommonDataModel_Wiki_Files/License.md", "sha256":"47916d758df3abf2a5a80d76dddf8632711a34cc2865c8edf0b25116790a1c4a"},
    {"id":"target_field_documentation", "tag":"v5.4.1", "commit":"4432382693f4e824f2fd5e482692c33772b7568a", "path":"inst/csv/OMOP_CDMv5.4_Field_Level.csv", "sha256":"06882359e4cfacd7f421855c6b9d542625fceaceb92e26df6fef873514257e17"},
    {"id":"target_table_documentation", "tag":"v5.4.1", "commit":"4432382693f4e824f2fd5e482692c33772b7568a", "path":"inst/csv/OMOP_CDMv5.4_Table_Level.csv", "sha256":"fd77b570b1e55406d42f494bd9b240d330026b5866f6354064e979f6918286de"},
    {"id":"target_ddl", "tag":"v5.4.1", "commit":"4432382693f4e824f2fd5e482692c33772b7568a", "path":"inst/ddl/5.4/postgresql/OMOPCDM_postgresql_5.4_ddl.sql", "sha256":"e4b845ed6044ac0a735f2ceb1613ff4106caa933ef5dc5eee5ef3b6109fceea0"},
    {"id":"target_license", "tag":"v5.4.1", "commit":"4432382693f4e824f2fd5e482692c33772b7568a", "path":"LICENSE", "sha256":"c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4"},
]

def dump(path, obj):
    path.write_text(json.dumps(obj, indent=2, ensure_ascii=False) + "\n")

def sql_table(text):
    m = re.search(r"CREATE TABLE\s+(?:@cdmDatabaseSchema\.)?(visit_occurrence)\s*\((.*?)\)\s*;", text, re.I | re.S)
    assert m
    table = m.group(1)
    rows = []
    for part in m.group(2).split(","):
        r = re.fullmatch(r"\s*(\w+)\s+(INTEGER|DATE|TIMESTAMP|VARCHAR\s*\(\d+\))\s+(NOT NULL|NULL)\s*", part, re.I)
        assert r, part
        name, dtype, required = r.groups()
        dtype = re.sub(r"\s+", "", dtype).upper()
        enum = {"INTEGER":"integer", "DATE":"date", "TIMESTAMP":"timestamp"}.get(dtype, "text")
        rows.append({"name":f"{table}.{name}", "table":table, "field":name, "declared_type":dtype, "data_type":enum, "required":required.upper() == "NOT NULL"})
    return rows, m.group(0) + "\n"

def markdown_fields(text):
    fields = []
    for number, line in enumerate(text.splitlines(), 1):
        if not line.startswith("|"):
            continue
        cells = [c.strip() for c in line.strip("|").split("|")]
        if len(cells) == 5:
            assert cells[1] == ""
            del cells[1]
        assert len(cells) == 4, cells
        name, required, dtype, description = cells
        if required not in ("Yes", "No"):
            assert dtype in ("Yes", "No"), cells
            required, dtype = dtype, required
        fields.append({"field":name, "description":description, "documented_type":dtype, "documented_required":required, "documentation_line":number})
    return fields

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("--output-dir", type=pathlib.Path, default=pathlib.Path(__file__).resolve().parent)
    out=parser.parse_args().output_dir
    raw=out/"raw"; raw.mkdir(parents=True, exist_ok=True)
    content={}; provenance=[]
    for item in SOURCES:
        url="https://raw.githubusercontent.com/OHDSI/CommonDataModel/"+item["commit"]+"/"+urllib.parse.quote(item["path"])
        b=urllib.request.urlopen(url).read()
        assert hashlib.sha256(b).hexdigest()==item["sha256"], item["id"]
        content[item["id"]]=b.decode()
        provenance.append({**item,"url":url,"full_source_bytes":len(b)})
    source, sddl=sql_table(content["source_ddl"])
    target, tddl=sql_table(content["target_ddl"])
    sd=markdown_fields(content["source_visit_documentation"])
    td=[r for r in csv.DictReader(io.StringIO(content["target_field_documentation"])) if r["cdmTableName"]=="VISIT_OCCURRENCE"]
    tt=[r for r in csv.DictReader(io.StringIO(content["target_table_documentation"])) if r["cdmTableName"]=="VISIT_OCCURRENCE"]
    assert len(source)==len(target)==len(sd)==len(td)==17 and len(tt)==1
    contexts={"source":{"schema_version":"5.2.2","table":source[0]["table"],"table_description":content["source_visit_documentation"].split("\n\nField|",1)[0],"table_conventions":content["source_visit_documentation"].split("### Conventions",1)[1].strip(),"description_source":"source_visit_documentation","fields":[]},"target":{"schema_version":"5.4.1","table":target[0]["table"],"publisher_table_metadata":tt[0],"description_source":"target_table_documentation","fields":[]}}
    for p,d in zip(source,sd):
        assert p["field"]==d["field"]
        assert p["required"]==(d["documented_required"]=="Yes")
        contexts["source"]["fields"].append({**p,**d,"declaration_source":"source_ddl","description_source":"source_visit_documentation"})
    for p,d in zip(target,td):
        assert p["field"]==d["cdmFieldName"] and p["required"]==(d["isRequired"]=="Yes")
        contexts["target"]["fields"].append({**p,"description":d["userGuidance"],"etl_conventions":d["etlConventions"],"documented_type":d["cdmDatatype"],"publisher_field_metadata":d,"declaration_source":"target_ddl","description_source":"target_field_documentation"})
    # Published vocabulary illustrations, never fabricated or patient observations.
    source_examples=[int(n) for n in re.findall(r"concept_id=(\d+)",content["source_visit_documentation"])]
    target_examples=[int(n) for n in re.findall(r"https://athena\.ohdsi\.org/search-terms/terms/(\d+)",tt[0]["userGuidance"])]
    assert source_examples==[262]
    assert target_examples==[9201,9203,262,42898160,9202,581476,5083,581458,32036,581478,38004193]
    schemas={}
    for side,rows,examples in [("source",source,source_examples),("target",target,target_examples)]:
        schemas[side]={"fields":[{"name":r["name"],"data_type":r["data_type"],"samples":[str(value) for value in examples] if r["field"]=="visit_concept_id" else []} for r in rows]}
    sample_evidence={"status":"Schema-only with publisher vocabulary illustrations; no observed clinical records.","source":{"field":next(r["name"] for r in source if r["field"]=="visit_concept_id"),"document_id":"source_visit_documentation","quote":"To annotate this correctly, the visit concept \"Emergency Room and Inpatient Visit\" (concept_id=262) should be used.","values":source_examples},"target":{"field":next(r["name"] for r in target if r["field"]=="visit_concept_id"),"document_id":"target_table_documentation","cell":"VISIT_OCCURRENCE.userGuidance","basis":"The publisher lists named standard Visit-domain concept illustrations and their exact Athena concept URLs.","values":target_examples}}
    dump(out/"schemas.json",schemas);dump(out/"contexts.json",contexts);dump(out/"sample_evidence.json",sample_evidence)
    dump(out/"provenance.json",{"publisher":"OHDSI/CommonDataModel","publisher_url":"https://github.com/OHDSI/CommonDataModel","retrieved_utc":"2026-10-13","source_documents":provenance,"schema_only":True,"observed_records":False,"examples":"Illustrative vocabulary identifiers published in the source schema conventions and target table documentation; sample_evidence.json identifies every included value. Other field samples are empty.","identifier_policy":"Preserve each literal declared table/field spelling: source visit_occurrence and target VISIT_OCCURRENCE. The target @cdmDatabaseSchema token is the publisher's schema placeholder and is omitted. PostgreSQL resolves the unquoted uppercase name case-insensitively.","type_translation":{"INTEGER":"integer","DATE":"date","TIMESTAMP":"timestamp","VARCHAR(n)":"text","documented datetime":"physical TIMESTAMP; normalized timestamp"},"licenses":{"source_documentation":"Public-domain dedication / CC0-1.0, source_license","target_repository":"Apache-2.0, target_license"},"publisher_discrepancies":["Source Markdown has an extra blank cell in one row and transposes Type/Required in five rows; named descriptions and requiredness are validated against DDL.","Source conventions contain undeclared stale aliases discharge_disposition_concept_id and PRECEDING_VISIT_ID; these are retained in context prose but not fabricated as schema fields.","Admission/discharge vocabulary ranges evolve from Place of Service descriptions to standardized Visit-domain concepts. Field roles correspond; a data ETL may require vocabulary remapping.","Target end-date conventions contain date/datetime spelling and record-provenance examples; field-specific descriptions, foreign-key domains and DDL govern roles."]})
    (raw/"source_visit_documentation.md").write_text(content["source_visit_documentation"])
    (raw/"source_visit_ddl_excerpt.sql").write_text(sddl);(raw/"target_visit_ddl_excerpt.sql").write_text(tddl)
    dump(raw/"target_field_documentation_excerpt.json",td);dump(raw/"target_table_documentation_excerpt.json",tt)
    (raw/"source_documentation_license.md").write_text(content["source_license"])
    (raw/"target_repository_LICENSE.txt").write_text(content["target_license"])
    print("Extracted one complete schema pair with 17 source and 17 target fields; no answers read or produced.")

if __name__=="__main__":
    main()
