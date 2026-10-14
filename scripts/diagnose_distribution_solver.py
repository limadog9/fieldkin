"""Capture the actual pinned Distribution MILP, solver log, and independent checks.

Run once per process/reference (the imported upstream source is immutable):
  python scripts/diagnose_distribution_solver.py --dataset PATH --output DIR
  python scripts/diagnose_distribution_solver.py --reference-fixes --dataset PATH --output DIR

Uses the existing reference adapters. No matching formulas or solver options are
changed; logging is enabled. This is a diagnostic, not an equivalent-output
performance benchmark. Use an external process deadline for expensive inputs.
"""

import argparse
import hashlib
from fractions import Fraction
import json
import re
import subprocess
from pathlib import Path
import sys


def check_solution(positive, separation):
    """Check directed binary triangles and recompute the *directed* objective."""
    n = len(positive)
    if n != len(separation) or any(len(row) != n for row in positive + separation):
        raise ValueError("Clustering matrices must be square and equally sized")
    if any(separation[i][j] not in (0, 1) for i in range(n) for j in range(n) if i != j):
        raise ValueError("Nonbinary separation")
    for u in range(n):
        for v in range(n):
            for w in range(n):
                if len({u, v, w}) == 3 and separation[u][w] > separation[u][v] + separation[v][w]:
                    raise ValueError(f"Triangle violation: {u}, {v}, {w}")
    objective = sum(separation[i][j] if positive[i][j] else 1 - separation[i][j]
                    for i in range(n) for j in range(n) if i != j)
    remaining, clusters = set(range(n)), []
    while remaining:
        stack, cluster = [min(remaining)], []
        remaining.remove(stack[0])
        while stack:
            i = stack.pop()
            cluster.append(i)
            neighbours = {j for j in remaining if separation[i][j] == 0 or separation[j][i] == 0}
            remaining -= neighbours
            stack.extend(sorted(neighbours))
        clusters.append(sorted(cluster))
    return {"objective": objective, "feasible": True, "partitions": clusters,
            "variables": n * (n - 1), "triangles": n * (n - 1) * (n - 2)}


def certify_lower_bound(positive):
    """Verify a rational LP dual certificate, not just a solver's status string.

    For min c*x with A*x<=0, 0<=x<=1, a feasible dual has y<=0,
    lower>=0, upper<=0 and A.T*y+lower+upper=c. Its bound is sum(upper)
    plus the objective constant. Exact rational equality makes the certificate
    independent of HiGHS' numerical primal objective/termination claim.
    """
    from scipy.optimize import linprog
    from scipy.sparse import coo_matrix

    n = len(positive)
    arcs = [(i, j) for i in range(n) for j in range(n) if i != j]
    indices = {pair: i for i, pair in enumerate(arcs)}
    triangles = [(u, v, w) for u in range(n) for v in range(n)
                 for w in range(n) if len({u, v, w}) == 3]
    rows, columns, values = [], [], []
    for row, (u, v, w) in enumerate(triangles):
        for pair, coefficient in (((u, w), 1), ((u, v), -1), ((v, w), -1)):
            rows.append(row)
            columns.append(indices[pair])
            values.append(coefficient)
    matrix = coo_matrix((values, (rows, columns)), shape=(len(triangles), len(arcs))).tocsr()
    objective = [1 if positive[i][j] else -1 for i, j in arcs]
    result = linprog(objective, A_ub=matrix, b_ub=[0] * len(triangles), bounds=(0, 1), method="highs")
    if not result.success:
        raise ValueError(f"Independent LP bound unavailable: {result.message}")
    rational = lambda values: [Fraction(float(value)).limit_denominator(1_000_000) for value in values]
    dual, lower, upper = map(rational, (result.ineqlin.marginals, result.lower.marginals, result.upper.marginals))
    if any(value > 0 for value in dual + upper) or any(value < 0 for value in lower):
        raise ValueError("Invalid dual signs")
    stationarity = [a + b for a, b in zip(lower, upper)]
    for (u, v, w), multiplier in zip(triangles, dual):
        for pair, coefficient in (((u, w), 1), ((u, v), -1), ((v, w), -1)):
            stationarity[indices[pair]] += multiplier * coefficient
    if stationarity != objective:
        raise ValueError("Rational LP dual does not satisfy exact stationarity")
    bound = sum(upper) + sum(not positive[i][j] for i, j in arcs)
    def sparse(values):
        return [[i, value.numerator, value.denominator] for i, value in enumerate(values) if value]
    return {"exact_bound": str(bound), "verified_with_exact_rationals": True,
            "highs_message": result.message, "triangle_dual": sparse(dual),
            "lower_dual": sparse(lower), "upper_dual": sparse(upper)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dataset", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--reference-fixes", action="store_true")
    parser.add_argument("--native", type=Path, help="Report from the ignored native capture test")
    args = parser.parse_args()
    # The existing module selects and verifies the pinned reference using argv.
    import run_valentine_parity as reference
    import pulp
    from valentine.algorithms.distribution_based import discovery

    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    raw = args.dataset.read_bytes()
    data = json.loads(raw)
    captures, active = [], {}
    original_cluster = discovery.correlation_clustering_pulp
    original_solve = pulp.LpProblem.solve
    original_cbc = discovery.PULP_CBC_CMD

    def logged_cbc(**kwargs):
        return original_cbc(**kwargs, logPath=str(output / f"cbc-{len(captures)}.log"))

    def solve(model, solver=None, **kwargs):
        # Model contains the actual coefficients, bounds and all constraints.
        model.writeLP(str(output / f"problem-{len(captures)}.lp"), max_length=1000)
        if model.sense != pulp.LpMinimize or any(variable.cat != pulp.LpInteger
                or variable.lowBound != 0 or variable.upBound != 1 for variable in model.variables()):
            raise ValueError("Unexpected solver objective sense or binary bounds")
        capture = {"problem": model.toDict()}
        active["capture"] = capture
        active["model"] = model
        result = original_solve(model, solver, **kwargs)
        capture.update(status=pulp.LpStatus[model.status], solution_status=pulp.LpSolution[model.sol_status],
                       solver_objective=pulp.value(model.objective),
                       solver_command=str(solver.path), solver_options=solver.options)
        return result

    def cluster(vertices, edges):
        ordered = list(vertices)
        values = original_cluster(vertices, edges)
        capture = active.pop("capture")
        model = active.pop("model")
        positive = [[i != j and edges[a][b] == 1 for j, b in enumerate(ordered)]
                    for i, a in enumerate(ordered)]
        separation = [[None if i == j else values[a, b] for j, b in enumerate(ordered)]
                      for i, a in enumerate(ordered)]
        # Decode each returned arc to the *actual* solver variable. The original
        # source embeds tuples, while the corrected source uses numeric IDs.
        def variable_name(i, j):
            if args.reference_fixes:
                return f"x_{i:08d}_{j:08d}"
            encoded = "({},{})".format(*[str(v).replace(" ", "__WHITESPACE__")
                .replace("-", "__DASH__").replace(">", "__GREATER__")
                .replace("/", "__BACKSLASH__") for v in (ordered[i], ordered[j])])
            return pulp.LpVariable(encoded).name
        by_name = {variable_name(i, j): (i, j) for i in range(len(ordered))
                   for j in range(len(ordered)) if i != j}
        if len(by_name) != len(values) or set(by_name) != set(model.variablesDict()):
            raise ValueError("Solver variable identities differ from input arcs")
        actual_objective = {(i, j): float(model.objective[model.variablesDict()[name]])
                            for name, (i, j) in by_name.items()}
        expected_objective = {(i, j): 1.0 if positive[i][j] else -1.0
                              for i, j in by_name.values()}
        expected_triangles = {tuple(sorted(((u, w, 1.0), (u, v, -1.0), (v, w, -1.0))))
                             for u in range(len(ordered)) for v in range(len(ordered))
                             for w in range(len(ordered)) if len({u, v, w}) == 3}
        actual_triangles = set()
        for constraint in model.constraints.values():
            if constraint.sense != -1 or constraint.constant != 0:
                raise ValueError("Unexpected triangle sense or constant")
            actual_triangles.add(tuple(sorted((*by_name[var.name], float(coefficient))
                                              for var, coefficient in constraint.items())))
        constant = sum(not positive[i][j] for i, j in by_name.values())
        if (actual_objective != expected_objective or model.objective.constant != constant
                or actual_triangles != expected_triangles
                or len(model.constraints) != len(expected_triangles)):
            raise ValueError("Actual MILP differs from directed sign/triangle formulation")
        certificate = certify_lower_bound(positive)
        (output / f"lp-certificate-{len(captures)}.json").write_text(json.dumps(certificate) + "\n")
        capture.update(vertices=ordered, positive=positive, separation=separation,
                       exact_lp_lower_bound=certificate["exact_bound"],
                       checks={**check_solution(positive, separation), "construction_verified": True})
        captures.append(capture)
        return values

    discovery.PULP_CBC_CMD = logged_cbc
    discovery.correlation_clustering_pulp = cluster
    pulp.LpProblem.solve = solve
    try:
        matches = reference.DistributionBased().get_matches(
            reference.InputTable("source", data["source"]["fields"]),
            reference.InputTable("target", data["target"]["fields"]))
    finally:
        discovery.PULP_CBC_CMD = original_cbc
        discovery.correlation_clustering_pulp = original_cluster
        pulp.LpProblem.solve = original_solve
    report = {"input_sha256": hashlib.sha256(raw).hexdigest(), "reference": reference.REFERENCE_METADATA,
              "python_version": sys.version, "pulp_version": pulp.__version__,
              "cbc_version": subprocess.check_output([captures[0]["solver_command"], "-stop"], text=True),
              "cbc_sha256": hashlib.sha256(Path(captures[0]["solver_command"]).read_bytes()).hexdigest(),
              "runtime_dependencies": {name: {"version": sys.modules[name].__version__,
                  "source_sha256": hashlib.sha256(Path(sys.modules[name].__file__).read_bytes()).hexdigest()}
                  for name in ("numpy", "scipy", "networkx", "pulp", "ot")},
              "problems": captures,
              "matches": reference.format_matches(matches)}
    if args.native:
        native_raw = args.native.read_bytes()
        native = json.loads(native_raw)
        if native["input_sha256"] != report["input_sha256"] or len(captures) != 1 or len(native["problems"]) != 1:
            raise ValueError("Native capture input/component mismatch")
        actual, expected = native["problems"][0], captures[0]
        if native["vertices"] != [[v[0], v[2]] for v in expected["vertices"]] or actual["positive"] != expected["positive"]:
            raise ValueError("Native graph identities/order/signs differ from upstream")
        checks = check_solution(actual["positive"], actual["separation"])
        model_counts = {key: int(re.search(rf"{key}: (\d+)", actual["model"]).group(1))
                        for key in ("num_vars", "num_constraints")}
        if model_counts != {"num_vars": checks["variables"], "num_constraints": checks["triangles"]}:
            raise ValueError("Actual native solver model counts differ from directed formulation")
        constant = sum(not row[j] for i, row in enumerate(actual["positive"]) for j in range(len(row)) if i != j)
        if (actual["status"] != "Optimal" or actual["termination_reason"] != "ProvenOptimal" or actual["gap"] != 0
                or actual["best_bound"] + constant != checks["objective"]
                or Fraction(expected["exact_lp_lower_bound"]) != checks["objective"]):
            raise ValueError("Native solution lacks matching optimality evidence")
        def pair_ids(matches):
            return {(m["pair"]["source_table"], m["pair"]["source_column"],
                     m["pair"]["target_table"], m["pair"]["target_column"]) for m in matches}
        for captured, matches in ((checks, native["matches"]), (expected["checks"], report["matches"])):
            pairs = set()
            for partition in captured["partitions"]:
                for i in partition:
                    for j in partition:
                        a, b = native["vertices"][i], native["vertices"][j]
                        if a[0] == "source" and b[0] == "target":
                            pairs.add((*a, *b))
            if pairs != pair_ids(matches):
                raise ValueError("Returned column pairs differ from weak zero-edge partitions")
        report["native_checks"] = {**checks, **model_counts, "capture_sha256": hashlib.sha256(native_raw).hexdigest(),
            "status": actual["status"], "termination_reason": actual["termination_reason"],
            "best_bound_without_constant": actual["best_bound"], "matching_pairs": len(native["matches"]),
            "different_directed_arcs": sum(actual["separation"][i][j] != expected["separation"][i][j]
                for i in range(len(actual["positive"])) for j in range(len(actual["positive"])) if i != j)}
    (output / "diagnostic.json").write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    print(json.dumps({"input_sha256": report["input_sha256"], "matches": len(report["matches"]),
                      "problems": [{"status": c["status"], "solution_status": c["solution_status"],
                                    **c["checks"]} for c in captures]}))


if __name__ == "__main__":
    main()
