#!/usr/bin/env python3
"""Assemble a standard case from frozen publisher schemas and sealed labels."""
import argparse, json, pathlib

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("--directory",type=pathlib.Path,default=pathlib.Path(__file__).resolve().parent)
    root=parser.parse_args().directory
    schemas=json.loads((root/"schemas.json").read_text())
    answers=json.loads((root/"labels.json").read_text())
    src=[f["name"] for f in schemas["source"]["fields"]]
    dest={f["name"] for f in schemas["target"]["fields"]}
    assert [a["source"] for a in answers]==src
    for a in answers:
        assert a["kind"] in ("match","no_match","ambiguous")
        if a["kind"]=="match":assert a["target"] in dest
        if a["kind"]=="ambiguous":assert len(a["targets"])>=2 and set(a["targets"])<=dest
    case={"name":"omop_v5_2_2_visit_occurrence_to_v5_4_1_visit_occurrence",**schemas,"answers":answers}
    (root/"case.json").write_text(json.dumps(case,indent=2)+'\n')
    print("Assembled one complete case with 17 source and 17 target fields.")

if __name__=="__main__":main()
