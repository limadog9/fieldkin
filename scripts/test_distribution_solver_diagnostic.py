"""Dependency-free checks of independent directed clustering diagnostics."""
import copy
import json
from pathlib import Path
import unittest

from diagnose_distribution_solver import check_lower_bound, check_solution


class DirectedSolutionTests(unittest.TestCase):
    def test_asymmetric_zero_edges_form_weak_components(self):
        positive = [[False, True, False], [True, False, True], [False, True, False]]
        separation = [[None, 1, 1], [0, None, 1], [0, 0, None]]
        checked = check_solution(positive, separation)
        self.assertEqual(checked, {"objective": 3, "feasible": True,
                                  "partitions": [[0, 1, 2]], "variables": 6, "triangles": 6})

    def test_empty_and_singleton(self):
        self.assertEqual(check_solution([], [])["partitions"], [])
        self.assertEqual(check_solution([[False]], [[None]])["objective"], 0)

    def test_infeasible_triangle_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "Triangle violation"):
            check_solution([[False] * 3 for _ in range(3)],
                           [[None, 0, 1], [0, None, 0], [1, 0, None]])

    def test_nonbinary_and_malformed_solutions_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "Nonbinary"):
            check_solution([[False, True], [True, False]], [[None, .5], [0, None]])
        with self.assertRaisesRegex(ValueError, "square"):
            check_solution([[False]], [[None, 0]])


class RecordedSolverEvidenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path = Path(__file__).resolve().parents[1] / "validation/distribution_solver_diagnosis.json"
        cls.cases = json.loads(path.read_text())["cases"]

    def assert_case(self, case):
        self.assertEqual(set(case["python"]), {"upstream", "fixed"})
        self.assertTrue(all(set(row) <= {"0", "1"} for row in case["positive_rows"]))
        positive = [[bit == "1" for bit in row] for row in case["positive_rows"]]
        bound = check_lower_bound(positive, case["rational_dual_certificate"])
        for report in [case["native"], *case["python"].values()]:
            separation = [[None if bit == "-" else int(bit) for bit in row]
                          for row in report["separation_rows"]]
            checked = check_solution(positive, separation)
            self.assertEqual(checked["objective"], bound, "feasible solution must attain the proven bound")
            for key, value in checked.items():
                self.assertEqual(value, report["checks"][key], key)
            expected_pairs = {(i, j) for cluster in checked["partitions"] for i in cluster for j in cluster
                              if case["vertices"][i][0] == "source" and case["vertices"][j][0] == "target"}
            actual_pairs = [(i, j) for i, j, _ in report["output"]]
            self.assertEqual(set(actual_pairs), expected_pairs)
            self.assertEqual(len(actual_pairs), len(expected_pairs), "duplicate output pair")

    def test_committed_ring_evidence_proves_both_solvers_optimal(self):
        self.assertEqual([case["name"] for case in self.cases], ["ring8", "ring16"])
        for case in self.cases:
            with self.subTest(case=case["name"]):
                self.assert_case(case)

    def test_feasibility_alone_does_not_establish_optimality(self):
        case = copy.deepcopy(self.cases[0])
        n = len(case["positive_rows"])
        case["native"]["separation_rows"] = ["".join("-" if i == j else "1" for j in range(n))
                                               for i in range(n)]
        with self.assertRaisesRegex(AssertionError, "proven bound"):
            self.assert_case(case)

    def test_corrupted_dual_certificates_are_rejected(self):
        case = self.cases[0]
        positive = [[bit == "1" for bit in row] for row in case["positive_rows"]]
        for corruption, message in [("sign", "signs"), ("coefficient", "stationarity"),
                                    ("bound", "recorded bound"), ("duplicate", "sparse"),
                                    ("index", "sparse"), ("denominator", "sparse")]:
            with self.subTest(corruption=corruption):
                certificate = copy.deepcopy(case["rational_dual_certificate"])
                entry = certificate["triangle_dual"][0]
                if corruption == "sign":
                    entry[1] = 1
                elif corruption == "coefficient":
                    entry[1] -= entry[2]
                elif corruption == "bound":
                    certificate["exact_bound"] = "65"
                elif corruption == "duplicate":
                    certificate["triangle_dual"].append(entry.copy())
                elif corruption == "index":
                    entry[0] = -1
                else:
                    entry[2] = 0
                with self.assertRaisesRegex(ValueError, message):
                    check_lower_bound(positive, certificate)


if __name__ == "__main__":
    unittest.main()
