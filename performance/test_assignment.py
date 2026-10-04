"""Small protocol checks; no compiler, benchmark measurements or network calls."""

import argparse
import csv
import io
import pathlib
import tempfile
import unittest

import assignment


def fixture(smoke=False):
    stream = io.StringIO()
    stream.write("\n".join(assignment.PREAMBLE) + "\n")
    writer = csv.DictWriter(stream, fieldnames=assignment.HEADER)
    writer.writeheader()
    for size, iterations in assignment.ITERATIONS.items():
        for family in assignment.FAMILIES:
            n = size // 4 if family == "wide-complete" else size
            m = size // 4 if family == "tall-bounded" else size
            confirmed = {"mixed-bounded": size // 4, "reserved-complete": size - 4}.get(family, 0)
            excluded = {"half-excluded-disabled": size // 2, "all-excluded-complete": size,
                        "mixed-bounded": size // 4}.get(family, 0)
            proposed = {"dense-disabled": size, "dense-bounded": size, "sparse-complete": 4,
                        "half-excluded-disabled": size // 2, "all-excluded-complete": 0,
                        "mixed-bounded": size // 2, "reserved-complete": 4,
                        "wide-complete": 4, "tall-bounded": 4}[family]
            solves = 0 if family.endswith("disabled") else 2 if family.endswith("bounded") else proposed
            status = "Disabled" if family.endswith("disabled") else "BudgetExhausted" if family.endswith("bounded") else "Complete"
            calls = 1 if smoke else iterations
            writer.writerow(dict(zip(assignment.HEADER, [family, size, n, m, calls, 1.0, 1000 / calls,
                             proposed, confirmed, excluded, n - proposed - confirmed, m - proposed - confirmed,
                             status, solves, solves * n * n * (m + n), 0,
                             "none" if status == "Disabled" else 0.85 * proposed])))
    return stream.getvalue()


class AssignmentProtocolTests(unittest.TestCase):
    def test_fixed_workload_inventory_and_smoke(self):
        for smoke in [False, True]:
            _, rows, behavior = assignment.validate(fixture(smoke), smoke=smoke)
            self.assertEqual(len(rows), 27)
            self.assertEqual(len(behavior), 27)
        with self.assertRaises(ValueError):
            assignment.validate(fixture(True))

    def test_rejects_missing_duplicate_reordered_and_malformed_workloads(self):
        lines = fixture().splitlines()
        variants = [lines[:-1], lines + [lines[-1]], lines[:3] + list(reversed(lines[3:])),
                    ["wrong preamble"] + lines[1:], lines[:3] + [lines[3] + ",unexpected"] + lines[4:]]
        for variant in variants:
            with self.subTest(variant=variant[-1]):
                with self.assertRaises(ValueError):
                    assignment.validate("\n".join(variant))

    def test_rejects_wrong_decisions_status_work_objective_and_duration(self):
        lines = fixture().splitlines()
        # A bounded workload must remain incomplete and retain its original-dimensional charge.
        for key, value in [("proposed", "0"), ("solves", "1"), ("work", "128"),
                           ("status", "Complete"), ("objective", "nan"),
                           ("alternatives", "3"), ("us_per_match", "inf"),
                           ("total_ms", "-1"), ("us_per_match", "100000")]:
            row = lines[4].split(",")
            row[assignment.HEADER.index(key)] = value
            with self.subTest(key=key):
                with self.assertRaises(ValueError):
                    assignment.validate("\n".join(lines[:4] + [",".join(row)] + lines[5:]))

    def test_recursive_inventory_detects_changes_and_additions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            for name in ["Cargo.toml", "Cargo.lock", "README.md", "evaluation/Cargo.toml",
                         *assignment.HARNESS, "src/lib.rs", "src/private/data.txt"]:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("original", encoding="utf8")
            before = assignment.inventory(root)
            (root / "src/private/data.txt").write_text("changed", encoding="utf8")
            self.assertNotEqual(before, assignment.inventory(root))
            (root / "src/private/data.txt").write_text("original", encoding="utf8")
            self.assertEqual(before, assignment.inventory(root))
            (root / "src/private/new.txt").write_text("new", encoding="utf8")
            self.assertNotEqual(before, assignment.inventory(root))
            (root / "src/private/new.txt").unlink()
            (root / "build.rs").write_text("fn main() {}", encoding="utf8")
            self.assertNotEqual(before, assignment.inventory(root))

    def test_refuses_existing_measurement_output_before_running_tools(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / "build.json").write_text("{}", encoding="utf8")
            (root / "baseline-run-1.csv").write_text("keep", encoding="utf8")
            with self.assertRaises(ValueError):
                assignment.measure(argparse.Namespace(output=root))
            self.assertEqual((root / "baseline-run-1.csv").read_text(encoding="utf8"), "keep")


if __name__ == "__main__":
    unittest.main()
