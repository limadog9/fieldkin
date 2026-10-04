"""Scale protocol checks without benchmark execution, builds or network access."""

import json
import pathlib
import tempfile
import unittest

import scale


def fixture(small=False):
    rows = []
    for size in ([128] if small else scale.SIZES):
        for family in scale.FAMILIES:
            for policy in [False, True]:
                unmatched = size // 8 if family == "partial" else 0
                rows.append({"family": family, "size": size, "one_to_one": policy,
                             "iterations": 3 if size == 128 else 1, "elapsed_ns": 123456,
                             "selected": size - unmatched, "unmatched": unmatched,
                             "pairs": size * size, "signal_evaluations": size * size * (3 if family == "builtin" else 1),
                             "diagnostic_solves": 0, "diagnostic_work": 0})
    return rows


def encode(rows):
    return "\n".join(json.dumps(row) for row in rows)


class ScaleProtocolTests(unittest.TestCase):
    def test_exact_inventory_and_small_functional_inventory(self):
        self.assertEqual(len(scale.validate(encode(fixture()))), 18)
        self.assertEqual(len(scale.validate(encode(fixture(True)), small=True)), 6)
        with self.assertRaises(ValueError):
            scale.validate(encode(fixture(True)))

    def test_rejects_missing_duplicate_or_reordered_workloads(self):
        rows = fixture()
        for changed in [rows[:-1], rows + [rows[-1]], list(reversed(rows))]:
            with self.assertRaises(ValueError):
                scale.validate(encode(changed))

    def test_rejects_wrong_decisions_budgets_types_and_durations(self):
        for key, value in [("selected", 0), ("unmatched", 1), ("pairs", 10),
                           ("signal_evaluations", 100), ("diagnostic_solves", 1),
                           ("diagnostic_work", 1), ("one_to_one", 1),
                           ("elapsed_ns", -1), ("elapsed_ns", True), ("size", 1024)]:
            rows = fixture()
            rows[0][key] = value
            with self.subTest(key=key, value=value):
                with self.assertRaises(ValueError):
                    scale.validate(encode(rows))

    def test_rejects_unexpected_output_fields(self):
        rows = fixture()
        rows[0]["sample_values"] = []
        with self.assertRaises(ValueError):
            scale.validate(encode(rows))

    def test_existing_output_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory)
            (path / "build.json").write_text("{}", encoding="utf8")
            (path / "run-1.jsonl").write_text("retained", encoding="utf8")
            with self.assertRaises(ValueError):
                scale.measure(path)
            self.assertEqual((path / "run-1.jsonl").read_text(encoding="utf8"), "retained")


if __name__ == "__main__":
    unittest.main()
