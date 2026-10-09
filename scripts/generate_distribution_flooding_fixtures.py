"""Regenerate native-port reference fixtures from the pinned Valentine checkout.

Run from the repository root after installing Valentine's Python dependencies.
The upstream checkout and optional test dependencies live under ignored .upstream.
Rust tests consume the resulting JSON without requiring Python at runtime.
"""

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT / ".upstream" / "python"), str(ROOT / ".upstream" / "valentine")]

from valentine.algorithms.distribution_based.distribution_based import DistributionBased
from valentine.algorithms.similarity_flooding import Formula, Policy, StringMatcher
from valentine.algorithms.similarity_flooding.similarity_flooding import SimilarityFlooding
from valentine.data_sources.base_column import BaseColumn
from valentine.data_sources.base_table import BaseTable


TYPE_NAMES = {"integer": "int", "float": "float", "decimal": "float", "date": "date",
              "timestamp": "date", "text": "varchar", "unknown": "varchar", "boolean": "varchar"}


class Column(BaseColumn):
    def __init__(self, definition, table):
        self.definition = definition
        self.table = table

    @property
    def unique_identifier(self):
        return f"{self.table}:{self.name}"

    @property
    def name(self):
        return self.definition["name"]

    @property
    def data_type(self):
        return TYPE_NAMES[self.definition["data_type"]]

    @property
    def data(self):
        return self.definition["samples"]


class InputTable(BaseTable):
    def __init__(self, definition):
        self.definition = definition

    @property
    def unique_identifier(self):
        return self.name

    @property
    def name(self):
        return self.definition["name"]

    def get_columns(self):
        return [Column(c, self.name) for c in self.definition["columns"]]

    def get_df(self):
        raise NotImplementedError("Fixtures expose columns directly")

    @property
    def is_empty(self):
        return not self.definition["columns"]


def table(name, columns):
    return {"name": name, "columns": [{"name": n, "data_type": t, "samples": list(map(str, s))}
                                      for n, t, s in columns]}


def scores(matches):
    return [{"pair": {"source_table": p.source_table, "source_column": p.source_column,
                      "target_table": p.target_table, "target_column": p.target_column},
             "score": float(score)} for p, score in sorted(matches.items())]


def main():
    fixtures = []
    schema_tables = [
        table("Employee", [("EmpNo", "integer", []), ("DeptName", "text", []),
                            ("Birthdate", "date", []), ("FirstName", "text", [])]),
        table("Department", [("DeptNo", "integer", []), ("DeptName", "text", []),
                              ("date", "date", []), ("Name", "text", [])]),
    ]
    for policy in Policy:
        for formula in Formula:
            for matcher in StringMatcher:
                config = {"policy": policy.name, "formula": formula.name, "string_matcher": matcher.name}
                algorithm = SimilarityFlooding(policy, formula, matcher)
                result = algorithm.get_matches(*(InputTable(t) for t in schema_tables))
                fixtures.append({"name": f"sf_{policy.name}_{formula.name}_{matcher.name}",
                                 "algorithm": "flooding", "tables": schema_tables, "config": config,
                                 "batch": False, "expected": scores(result)})
    extra = table("Payroll", [("EmployeeNo", "integer", []), ("DeptName", "text", []),
                              ("EmployeeName", "text", [])])
    for batch in [False, True]:
        algorithm = SimilarityFlooding(string_matcher=StringMatcher.PREFIX_SUFFIX_TFIDF,
                                       tfidf_corpus=[InputTable(extra)])
        tables = schema_tables + [extra] if batch else schema_tables
        result = algorithm.get_matches_batch([InputTable(t) for t in tables]) if batch else algorithm.get_matches(*(InputTable(t) for t in tables))
        fixtures.append({"name": f"sf_idf_{'batch' if batch else 'corpus'}", "algorithm": "flooding",
                         "tables": tables, "corpus": [extra], "config": {"string_matcher": "PREFIX_SUFFIX_TFIDF"},
                         "batch": batch, "expected": scores(result)})

    distributions = [
        ("dist_identical_and_disjoint", [
            table("z_source", [("id", "integer", range(1, 11)), ("city", "text", ["A", "B", "C", "D"]), ("empty", "text", [])]),
            table("a_target", [("identifier", "integer", range(1, 11)), ("location", "text", ["A", "B", "C", "D"]), ("shifted", "integer", range(100, 110))]),
        ], {}),
        ("dist_overlap_shift", [table("aaa", [("value", "integer", range(1, 11))]),
                                table("bbb", [("shift", "integer", range(2, 12))])], {"quantiles": 4}),
        ("dist_overlap_shift_transpose", [table("bbb", [("shift", "integer", range(2, 12))]),
                                          table("aaa", [("value", "integer", range(1, 11))])], {"quantiles": 4}),
        ("dist_single_quantile", [table("aaa", [("value", "integer", range(1, 11))]),
                                  table("bbb", [("shift", "integer", range(2, 12))])], {"quantiles": 1}),
        ("dist_constant", [table("aaa", [("same", "text", ["x", "x", "x"]), ("different", "text", ["z", "z"])]),
                           table("bbb", [("same2", "text", ["x", "x"]), ("nan", "float", ["NaN", "NaN"])])], {}),
        ("dist_duplicate_numeric_aliases", [table("aaa", [("value", "integer", ["1", "2", "3", "4", "5"])]),
                                            table("bbb", [("other", "float", ["1.0", "2", "3.0", "4", "5"])])], {"threshold1": 1.0, "threshold2": 1.0, "quantiles": 4}),
        ("dist_batch_global_ranks", schema_tables[:0] + [
            table("aaa", [("id", "integer", range(1, 11)), ("name", "text", ["Adam", "Bob", "Eve", "Tom"])]),
            table("bbb", [("no", "integer", range(2, 12)), ("person", "text", ["Adam", "Bob", "Eve", "Tom"])]),
            table("ccc", [("other_id", "integer", [1, 1, 2, 2, 5, 9]), ("first", "text", ["Adam", "Bob", "Eve", "Tom"])])
        ], {"quantiles": 8, "threshold1": 0.5, "threshold2": 0.5}),
        ("dist_attribute_integer_program", [
            table("aaa", [("a", "text", list("aaaabbcddd")), ("b", "text", list("aabbccddde")), ("c", "text", list("ddeeffggg"))]),
            table("bbb", [("x", "text", list("abbcccdddd")), ("y", "text", list("ccdddeeeee")), ("z", "text", list("eefffgggg"))]),
        ], {"quantiles": 8, "threshold1": 0.8, "threshold2": 0.1}),
    ]
    for name, tables, config in distributions:
        for bloom in [False, True]:
            actual_config = dict(config, use_bloom_filters=bloom)
            algorithm = DistributionBased(**actual_config)
            inputs = [InputTable(t) for t in tables]
            batch = len(inputs) > 2
            result = algorithm.get_matches_batch(inputs) if batch else algorithm.get_matches(*inputs)
            fixtures.append({"name": name + ("_bloom" if bloom else "_exact"), "algorithm": "distribution",
                             "tables": tables, "config": actual_config, "batch": batch, "expected": scores(result)})
    destination = ROOT / "tests" / "fixtures" / "distribution_flooding.json"
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(fixtures, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote {len(fixtures)} upstream fixtures to {destination}")


if __name__ == "__main__":
    main()
