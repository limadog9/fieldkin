"""Regression tests for the opt-in upstream Distribution identity fix.

Run `python scripts/test_distribution_reference_patch.py`. Requires the pinned
upstream checkout and dependencies used by scripts/run_valentine_parity.py.
The tests load the versioned patched source overlay, preserving the checkout.
"""

import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
from valentine_reference import prepare_reference

CHECKOUT_SOURCE = ROOT / ".upstream/valentine/valentine/algorithms/distribution_based/discovery.py"
ORIGINAL_SOURCE_HASH = hashlib.sha256(CHECKOUT_SOURCE.read_bytes()).hexdigest()
REFERENCE, METADATA = prepare_reference(fixes=True)
sys.path[:0] = [str(REFERENCE), str(ROOT / ".upstream/python")]

from valentine.algorithms.distribution_based import discovery
from valentine.algorithms.distribution_based.distribution_based import DistributionBased
from valentine.algorithms.match import ColumnPair
from valentine.data_sources.base_column import BaseColumn
from valentine.data_sources.base_table import BaseTable

if not Path(discovery.__file__).resolve().is_relative_to(REFERENCE.resolve()):
    raise ImportError("Distribution imported outside the patched reference")

TYPE_NAMES = {"integer": "int", "float": "float", "decimal": "float", "date": "date",
              "timestamp": "date", "text": "varchar", "boolean": "varchar", "unknown": "varchar"}


class InputColumn(BaseColumn):
    def __init__(self, table, definition, index):
        self.table = table
        self.definition = definition
        self.index = index

    @property
    def name(self):
        return self.definition["name"]

    @property
    def unique_identifier(self):
        return f"{self.table}:{self.index}:{self.name}"

    @property
    def data_type(self):
        return TYPE_NAMES[self.definition["data_type"]]

    @property
    def data(self):
        return self.definition.get("samples", [])


class InputTable(BaseTable):
    def __init__(self, name, definition):
        self.table_name = name
        self.columns = [InputColumn(name, field, index) for index, field in enumerate(definition["fields"])]

    @property
    def name(self):
        return self.table_name

    @property
    def unique_identifier(self):
        return self.name

    @property
    def is_empty(self):
        return all(column.is_empty for column in self.columns)

    def get_columns(self):
        return self.columns

    def get_df(self):
        raise AssertionError("Column adapters retain the original fixture samples")


class DistributionReferencePatchTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        temporary = ROOT / "target/parity/reference_patch_test_tmp"
        temporary.mkdir(parents=True, exist_ok=True)
        tempfile.tempdir = str(temporary)

    def test_solver_retains_original_tuple_keys_and_avoids_variable_name_collisions(self):
        # PuLP normally aliases [] and + to underscores. The old decoder also
        # modified literal replacement tags such as __WHITESPACE__ in user names.
        names = ["geometry.coordinates[0]", "geometry.coordinates_0_", "axis-0",
                 "axis+0", "axis_0", "literal__WHITESPACE__tag"]
        vertices = [("source[set]-a", "table+uid", name, f"column/{name}") for name in names]
        group = {vertex: index // 2 for index, vertex in enumerate(vertices)}
        edges = {u: {v: 1 if group[u] == group[v] else -1 for v in vertices if u != v} for u in vertices}
        solver_names = []
        original_solve = discovery.plp.LpProblem.solve

        def inspect_solve(model, *args, **kwargs):
            solver_names.extend(variable.name for variable in model.variables())
            return original_solve(model, *args, **kwargs)

        with patch.object(discovery.plp.LpProblem, "solve", inspect_solve):
            result = discovery.correlation_clustering_pulp(vertices, edges)

        expected_pairs = {(u, v) for u in vertices for v in vertices if u != v}
        self.assertEqual(set(result), expected_pairs)
        self.assertEqual(len(solver_names), len(expected_pairs))
        self.assertEqual(len(set(solver_names)), len(solver_names))
        self.assertTrue(all(name.startswith("x_") for name in solver_names))
        # Three separate positive cliques have a unique zero-cost solution. This
        # checks the actual directed binary objective and clustering behavior.
        for (u, v), value in result.items():
            self.assertEqual(value, 0.0 if group[u] == group[v] else 1.0)
        objective = sum(value if edges[u][v] == 1 else 1.0 - value for (u, v), value in result.items())
        self.assertEqual(objective, 0.0)
        clusters = discovery.process_correlation_clustering_result([result], vertices)
        self.assertEqual({frozenset(cluster) for cluster in clusters},
                         {frozenset(vertices[start:start + 2]) for start in range(0, len(vertices), 2)})

    def test_usgs_original_coordinate_identifiers_and_scores_are_preserved(self):
        fixture = json.loads((ROOT / "eval_realworld/usgs_earthquakes.json").read_text(encoding="utf8"))
        source = InputTable("source", fixture["source"])
        target = InputTable("target", fixture["target"])
        matches = DistributionBased().get_matches(source, target)
        self.assertEqual(len(matches), 17)
        expected = [
            ("longitude", "geometry.coordinates[0]", 0.9796141607352509),
            ("latitude", "geometry.coordinates[1]", 0.9635667691697093),
            ("depth", "geometry.coordinates[2]", 0.9890249404851382),
        ]
        for source_name, target_name, score in expected:
            self.assertEqual(matches[ColumnPair("source", source_name, "target", target_name)], score)
        source_names = {column.name for column in source.columns}
        target_names = {column.name for column in target.columns}
        for pair in matches:
            self.assertIn(pair.source_column, source_names)
            self.assertIn(pair.target_column, target_names)

    def test_original_upstream_checkout_remains_unchanged(self):
        self.assertEqual(hashlib.sha256(CHECKOUT_SOURCE.read_bytes()).hexdigest(), ORIGINAL_SOURCE_HASH)


if __name__ == "__main__":
    unittest.main()
