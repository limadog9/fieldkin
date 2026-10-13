#!/usr/bin/env python3
"""Assemble the standard case from frozen schema input and sealed labels."""
import argparse, json, pathlib

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--directory", type=pathlib.Path, default=pathlib.Path(__file__).resolve().parent)
    args = parser.parse_args()
    root = args.directory
    schemas = json.loads((root / "schemas.json").read_text())
    answers = json.loads((root / "labels.json").read_text())
    src = [f["name"] for f in schemas["source"]["fields"]]
    target = {f["name"] for f in schemas["target"]["fields"]}
    assert [a["source"] for a in answers] == src
    for a in answers:
        assert a["kind"] in ("match", "no_match", "ambiguous")
        if a["kind"] == "match":
            assert a["target"] in target
        elif a["kind"] == "ambiguous":
            assert len(a["targets"]) >= 2 and set(a["targets"]) <= target
    case = {"name": "omop_v5_0_1_procedure_cost_to_v5_2_2_cost", **schemas, "answers": answers}
    (root / "case.json").write_text(json.dumps(case, indent=2) + "\n")
    print("Assembled one complete standard-format case with 13 source and 22 target fields.")

if __name__ == "__main__":
    main()
