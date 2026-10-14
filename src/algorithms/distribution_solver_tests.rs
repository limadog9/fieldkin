//! Test-only capture of the actual directed MILP; no alternate production solver.
use super::*;
use good_lp::solvers::microlp::{MicroLpProblem, MicroLpSolution};
use std::cell::RefCell;

thread_local! {
    static CAPTURE: RefCell<Option<Vec<serde_json::Value>>> = const { RefCell::new(None) };
    static PROBLEM: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub(super) fn record_problem(model: &MicroLpProblem) {
    if CAPTURE.with_borrow(Option::is_some) {
        PROBLEM.with_borrow_mut(|problem| *problem = Some(format!("{:?}", model.as_inner())));
    }
}

pub(super) fn record(
    positive: &[Vec<bool>],
    variables: &[Vec<Option<good_lp::Variable>>],
    solution: MicroLpSolution,
) {
    CAPTURE.with_borrow_mut(|capture| {
        let Some(capture) = capture else { return };
        let separation: Vec<Vec<_>> = variables
            .iter()
            .map(|row| row.iter().map(|v| v.map(|v| solution.value(v))).collect())
            .collect();
        let status = format!("{:?}", solution.status());
        let inner = solution.into_inner();
        let stats = inner.stats();
        capture.push(serde_json::json!({
            "positive": positive,
            "model": PROBLEM.with_borrow_mut(Option::take),
            "separation": separation,
            "status": status,
            "termination_reason": format!("{:?}", inner.termination_reason()),
            "linear_objective": inner.objective(),
            "best_bound": stats.best_bound,
            "gap": stats.gap,
            "nodes_solved": stats.nodes_solved,
            "lp_iterations": stats.lp_iterations,
        }));
    });
}

fn solve_and_check(graph: &[Vec<bool>]) -> (Vec<Vec<usize>>, usize) {
    CAPTURE.with_borrow_mut(|capture| *capture = Some(Vec::new()));
    let clusters = correlation_clusters(graph, None).unwrap();
    let captured = CAPTURE.with_borrow_mut(|capture| capture.take().unwrap());
    assert_eq!(captured.len(), 1);
    let problem = &captured[0];
    assert_eq!(problem["status"], "Optimal");
    assert_eq!(problem["termination_reason"], "ProvenOptimal");
    assert_eq!(problem["gap"], 0.0);
    let n = graph.len();
    let value = |i: usize, j: usize| problem["separation"][i][j].as_f64().unwrap();
    let mut objective = 0;
    let mut joined = vec![vec![false; n]; n];
    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }
            let x = value(i, j);
            assert!(x == 0.0 || x == 1.0);
            objective += usize::from((x == 1.0) == graph[i][j]);
            joined[i][j] = x == 0.0;
            for k in 0..n {
                if k != i && k != j {
                    assert!(value(i, k) <= x + value(j, k));
                }
            }
        }
    }
    // good_lp removes the objective constant before handing it to microlp.
    let constant = graph
        .iter()
        .enumerate()
        .map(|(i, row)| {
            row.iter()
                .enumerate()
                .filter(|&(j, positive)| i != j && !positive)
                .count()
        })
        .sum::<usize>();
    assert_eq!(
        problem["best_bound"].as_f64().unwrap() + constant as f64,
        objective as f64
    );
    assert_eq!(clusters, connected_components(&joined));
    (clusters, objective)
}

#[test]
fn conflicting_directed_optima_are_feasible_and_deterministic() {
    // Two copies of a four-cycle. Directed triangle constraints intentionally
    // allow asymmetric optimal matrices; weak zero-edge components need not be
    // the symmetric partition that another integer solver happens to choose.
    let graph: Vec<Vec<bool>> = (0..8)
        .map(|i| {
            (0..8)
                .map(|j| i != j && ((i % 4 + 4 - j % 4) % 4 != 2))
                .collect()
        })
        .collect();
    let first = solve_and_check(&graph);
    assert_eq!(first, solve_and_check(&graph));
    // Bounded independent enumeration of every directed binary assignment on
    // three nodes tests the objective instead of trusting equal solver scores.
    for mask in 0_u32..64 {
        let mut graph = vec![vec![false; 3]; 3];
        let edges: Vec<_> = (0..3)
            .flat_map(|i| (0..3).filter(move |&j| i != j).map(move |j| (i, j)))
            .collect();
        for (bit, &(i, j)) in edges.iter().enumerate() {
            graph[i][j] = mask & (1 << bit) != 0;
        }
        if connected_components(&graph)
            .iter()
            .all(|c| c.iter().all(|&i| c.iter().all(|&j| i == j || graph[i][j])))
        {
            continue; // These use the independently covered zero-cost shortcut.
        }
        let expected = (0_u32..64)
            .filter_map(|candidate| {
                let mut x = [[0_usize; 3]; 3];
                for (bit, &(i, j)) in edges.iter().enumerate() {
                    x[i][j] = usize::from(candidate & (1 << bit) != 0);
                }
                if (0..3).any(|i| {
                    (0..3).any(|j| {
                        (0..3).any(|k| i != j && j != k && i != k && x[i][k] > x[i][j] + x[j][k])
                    })
                }) {
                    return None;
                }
                Some(
                    edges
                        .iter()
                        .map(|&(i, j)| usize::from((x[i][j] == 1) == graph[i][j]))
                        .sum::<usize>(),
                )
            })
            .min()
            .unwrap();
        assert_eq!(solve_and_check(&graph).1, expected);
    }
}

#[test]
#[ignore = "manual solver diagnostic; requires FIELDKIN_DISTRIBUTION_INPUT and OUTPUT"]
fn capture_distribution_problem() {
    let path = std::env::var("FIELDKIN_DISTRIBUTION_INPUT").unwrap();
    let raw = std::fs::read(path).unwrap();
    let input: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let source = Table::from_schema(
        "source",
        &serde_json::from_value(input["source"].clone()).unwrap(),
    );
    let target = Table::from_schema(
        "target",
        &serde_json::from_value(input["target"].clone()).unwrap(),
    );
    CAPTURE.with_borrow_mut(|capture| *capture = Some(Vec::new()));
    let matches = DistributionBased::default()
        .get_matches(&source, &target)
        .unwrap();
    let problems = CAPTURE.with_borrow_mut(|capture| capture.take().unwrap());
    assert_eq!(
        problems.len(),
        1,
        "manual diagnostic expects one full ring component"
    );
    let mut vertices: Vec<_> = [&source, &target]
        .into_iter()
        .flat_map(|table| {
            table
                .columns
                .iter()
                .map(|column| (table.name.clone(), column.name.clone()))
        })
        .collect();
    vertices.sort();
    assert_eq!(
        problems[0]["positive"].as_array().unwrap().len(),
        vertices.len()
    );
    let report = serde_json::json!({
        "input_sha256": format!("{:x}", Sha256::digest(&raw)),
        "problems": problems,
        "vertices": vertices,
        "matches": matches.iter().map(|(pair, score)| serde_json::json!({"pair":pair,"score":score})).collect::<Vec<_>>(),
    });
    std::fs::write(
        std::env::var("FIELDKIN_DISTRIBUTION_OUTPUT").unwrap(),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}
