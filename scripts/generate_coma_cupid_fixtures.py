"""Regenerate COMA/Cupid golden tests from the pinned Valentine checkout.

Verification tooling only. Requires the upstream Python dependencies and NLTK
English stopwords, WordNet 3.0 and punkt_tab in .upstream/nltk_data. Rust does
not execute this script or require any Python installation.
"""

import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
sys.path[:0] = [str(ROOT / ".upstream/python"), str(ROOT / ".upstream/valentine")]
import nltk
nltk.data.path.insert(0, str(ROOT / ".upstream/nltk_data"))
from valentine.algorithms import Coma, Cupid
from valentine.algorithms.cupid.linguistic_matching import _token_similarity
from valentine.algorithms.cupid.tree_match import tree_match
from valentine.data_sources.base_table import BaseTable
from valentine.data_sources.base_column import BaseColumn

TYPES = {"unknown": "unknown", "boolean": "boolean", "integer": "int", "float": "float",
         "decimal": "decimal", "text": "varchar", "date": "date", "timestamp": "timestamp"}


def field(name, samples=(), dtype="text"):
    return {"name": name, "data_type": dtype, "samples": list(samples)}


def table(name, *fields):
    return {"name": name, "columns": list(fields)}


class Column(BaseColumn):
    def __init__(self, data):
        self.item = data

    @property
    def unique_identifier(self):
        return self.name

    @property
    def name(self):
        return self.item["name"]

    @property
    def data_type(self):
        return TYPES[self.item["data_type"]]

    @property
    def data(self):
        return self.item["samples"]


class InputTable(BaseTable):
    def __init__(self, data):
        self.item = data
        self.columns = [Column(f) for f in data["columns"]]

    @property
    def unique_identifier(self):
        return self.name

    @property
    def name(self):
        return self.item["name"]

    @property
    def is_empty(self):
        return not self.columns

    def get_columns(self):
        return self.columns

    def get_instances_columns(self):
        return self.columns

    def get_df(self):
        raise AssertionError("Fixture algorithms must use columns directly")


def entries(matches, details=None):
    output = []
    for p, score in sorted(matches.items(), key=lambda pair: (
        pair[0].source_table, pair[0].source_column, pair[0].target_table, pair[0].target_column)):
        item = {"pair": {"source_table": p.source_table, "source_column": p.source_column,
                         "target_table": p.target_table, "target_column": p.target_column},
                "score": float(score)}
        if details is not None:
            item["details"] = {k: float(v) for k, v in details.get(p, {}).items()}
        output.append(item)
    return output


def main():
    names = [table("s", field("ApproxDate"), field("Dept"), field("personName"), field("mgr")),
             table("t", field("date_created_approximation"), field("department"),
                   field("name_person"), field("manager"))]
    values = [table("s", field("left", ["red apple", "green pear", "green pear", "the and or", ""]),
                    field("number", ["42", "43", "44"], "integer")),
              table("t", field("right", ["red apple", "yellow banana"]),
                    field("identifier", ["42", "43", "44"], "integer"))]
    batch = [table("s", field("a", ["common rare"])),
             table("t", field("b", ["common other"])),
             table("u", field("c", ["common", "common", "extra"]))]
    ties = [table("s", field("account"), field("account_name")),
            table("t", field("accountName"), field("Account_Name"), field("account"))]
    semantic = [table("source", field("car"), field("physician"), field("running"),
                      field("quick"), field("cats"), field("best")),
                table("target", field("automobile"), field("doctor"), field("walking"),
                      field("fast"), field("dogs"), field("good"))]
    typed = [table("source", field("car12", dtype="integer"), field("value", dtype="float"),
                   field("event_date", dtype="date"), field("theName", dtype="text")),
             table("target", field("automobile13", dtype="integer"), field("value", dtype="integer"),
                   field("eventDate", dtype="timestamp"), field("name", dtype="text"))]
    incompatible_shape = [table("s", field("alpha")),
                          table("t", field("alpha"), field("beta"), field("gamma"))]

    cases = []
    for name, tables, kwargs in [
        ("schema_default", names, {}),
        ("schema_all", names, {"delta": 0.0}),
        ("schema_threshold", names, {"threshold": 0.7, "delta": 0.0}),
        ("schema_max_n_ties", ties, {"max_n": 1, "delta": 0.0}),
        ("combined_weighted", values, {"use_instances": True, "instance_weight": 2.5, "delta": 0.0}),
        ("instance_only", values, {"use_schema": False, "use_instances": True, "delta": 0.0}),
        ("global_batch", batch, {"use_schema": False, "use_instances": True, "delta": 0.0}),
        ("pairwise_idf_differs", batch[:2], {"use_schema": False, "use_instances": True, "delta": 0.0}),
        ("stopword_values", [table("s", field("a", ["the and or"])),
                             table("t", field("b", ["was is"]))],
            {"use_schema": False, "use_instances": True}),
        ("common_idf_zero", [table("s", field("a", ["same"])),
                             table("t", field("b", ["same"]))],
            {"use_schema": False, "use_instances": True}),
    ]:
        matcher = Coma(**kwargs)
        inputs = [InputTable(t) for t in tables]
        matches = matcher.get_matches_batch(inputs)
        cases.append({"algorithm": "coma", "name": name, "tables": tables, "config": kwargs,
                      "expected": entries(matches, matcher.match_details)})

    for name, tables, kwargs in [
        ("wordnet_default", semantic, {}),
        ("wordnet_all", semantic, {"th_accept": 0.0}),
        ("wordnet_parallel", semantic, {"process_num": 2}),
        ("typed_weighted", typed, {"th_accept": 0.0}),
        ("typed_strict_category", typed, {"th_ns": 1.0, "th_accept": 0.0}),
        ("reinforcement", semantic, {"w_struct": 0.9, "th_high": 0.4, "th_low": 0.0}),
        ("weighted_structure", semantic, {"leaf_w_struct": 0.6, "w_struct": 0.8,
                                           "th_accept": 0.3, "c_inc": 1.5, "c_dec": 0.7}),
        ("shape_ratio_skips_propagation", incompatible_shape, {"th_accept": 0.0}),
        ("punctuation_contractions", [table("s", field("HelloWorld, 123 and hello"),
                                            field("They'll save $3.88.")),
                                      table("t", field("hello world 124"), field("They will save 3.88"))],
            {"th_accept": 0.0}),
    ]:
        matcher = Cupid(**kwargs)
        source, target = [InputTable(t) for t in tables]
        matches = matcher.get_matches(source, target)
        # Include original leaf linguistic/structural values as well as mapping.
        source_tree = matcher._Cupid__schemata["DB__" + source.name]
        target_tree = matcher._Cupid__schemata["DB__" + target.name]
        parameters = dict(leaf_w_struct=0.2, w_struct=0.2, th_accept=0.7, th_high=0.6,
                          th_low=0.35, c_inc=1.2, c_dec=0.9, th_ns=0.7, process_num=1)
        parameters.update(kwargs)
        sims = tree_match(source_tree, target_tree, matcher._Cupid__categories, **parameters)
        details = {}
        from valentine.algorithms.match import ColumnPair
        for s in source_tree.get_leaves():
            for t in target_tree.get_leaves():
                key = ColumnPair(source.name, s.name, target.name, t.name)
                details[key] = sims[s.long_name, t.long_name]
        cases.append({"algorithm": "cupid", "name": name, "tables": tables, "config": kwargs,
                      "expected": entries(matches, details)})

    pairs = [("car", "automobile"), ("physician", "doctor"), ("dog", "cat"),
             ("cats", "dogs"), ("running", "walking"), ("eat", "consume"),
             ("big", "large"), ("best", "good"), ("quick", "fast"),
             ("employee", "worker"), ("department", "division"), ("12", "13"),
             ("zxqabc", "zxqabd"), ("physical_entity", "thing")]
    fixture = {"upstream_commit": subprocess.check_output(
        ["git", "-C", str(ROOT / ".upstream/valentine"), "rev-parse", "HEAD"], text=True).strip(),
               "wordnet_version": "3.0", "cases": cases,
               "word_pairs": [{"a": a, "b": b, "score": _token_similarity(a, b)} for a, b in pairs]}
    destination = ROOT / "tests/fixtures/coma_cupid.json"
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(fixture, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote {len(cases)} cases and {len(pairs)} semantic word pairs to {destination}")


if __name__ == "__main__":
    main()
