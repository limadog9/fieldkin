"""Dependency-free checks of independent directed clustering diagnostics."""
import unittest

from diagnose_distribution_solver import check_solution


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


if __name__ == "__main__":
    unittest.main()
