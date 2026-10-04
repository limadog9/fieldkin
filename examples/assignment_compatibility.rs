//! Deterministic public-API assignment compatibility transcript.
//!
//! Copy this same source into two revisions, run each in release mode with the
//! same compiler, and compare stdout byte for byte. Complete reports (including
//! scores, warnings, diagnostics and witnesses), errors and callback order are
//! emitted; there is no hash dependency or lossy projection of the report.
//! This is synthetic regression evidence, not an accuracy benchmark.
//! Use `--check-only` to execute the same assertions with a short summary.

use std::error::Error;
use std::io::{self, BufWriter, Write};
use std::sync::{Arc, Mutex};

use fieldkin::{
    BudgetKind, Config, DataType, Evidence, Field, FieldPair, GlobalDiagnosticsConfig,
    MatchConstraints, MatchEngine, MatchError, Matcher, Schema, WeightedMatcher,
};

const CASES: usize = 1_024;
const SEED: u64 = 20_261_004;
const EXPLANATION: &str = "Fixed synthetic compatibility evidence";
type Trace = Arc<Mutex<Vec<(String, String)>>>;

struct Generator(u64);

impl Generator {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn index(&mut self, bound: usize) -> usize {
        ((self.next() >> 32) % bound as u64) as usize
    }
}

struct MatrixMatcher {
    scores: Vec<Vec<Option<f64>>>,
    trace: Trace,
}

impl Matcher for MatrixMatcher {
    fn name(&self) -> &str {
        "compatibility-matrix"
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        // Only generated stable IDs are used. Trace instrumentation deliberately
        // observes callback ordering; it does not affect the returned evidence.
        let index = |field: &Field| {
            field.id.0[1..]
                .parse::<usize>()
                .map_err(|_| "Invalid generated field ID".to_owned())
        };
        let row = index(source)?;
        let column = index(target)?;
        self.trace
            .lock()
            .map_err(|_| "Trace mutex poisoned".to_owned())?
            .push((source.id.0.clone(), target.id.0.clone()));
        Ok(Evidence {
            score: self.scores[row][column],
            explanation: EXPLANATION.into(),
        })
    }
}

fn schema(prefix: &str, count: usize) -> Schema {
    Schema::new(
        (0..count)
            .map(|index| {
                Field::new(
                    format!("{prefix}{index:03}"),
                    "duplicate name",
                    DataType::Text,
                )
            })
            .collect(),
    )
}

fn scores(case: usize, rows: usize, columns: usize, rng: &mut Generator) -> Vec<Vec<Option<f64>>> {
    (0..rows)
        .map(|row| {
            (0..columns)
                .map(|column| {
                    let random = rng.index(16);
                    match case % 10 {
                        0 => None,
                        1 => (row == column && row % 2 == 1).then_some(0.75),
                        2 => (row % 2 == 1 && column % 2 == 1).then_some(0.5),
                        3 => (random % 3 != 0).then_some((random % 5) as f64 / 4.0),
                        4 => (random % 3 != 0)
                            .then_some(f64::from_bits(0.5_f64.to_bits() + random as u64)),
                        5 => (random % 3 != 0).then_some(f64::from_bits(random as u64)),
                        6 => Some(random as f64 / 15.0),
                        7 => (row != 0 && column != 0 && row.abs_diff(column) <= 1).then_some(0.5),
                        8 => (row % 3 == 1 && column % 3 == 2)
                            .then_some((random % 3 + 1) as f64 / 3.0),
                        _ => (random < 3).then_some(random as f64 / 2.0),
                    }
                })
                .collect()
        })
        .collect()
}

fn fractional_dead_row_fixture() -> Vec<Vec<Option<f64>>> {
    // The final row has no edge but still changes the original floating-point
    // Hungarian tie outcome. Dropping it would assign t002 to s000 instead of
    // s003. Keep this explicit counterexample alongside generated scenarios.
    vec![
        vec![None, None, Some(0.3)],
        vec![Some(1.0), None, None],
        vec![None, Some(0.1), Some(0.3)],
        vec![Some(0.7), None, Some(0.3)],
        vec![None, None, None],
    ]
}

fn constraints(case: usize, rows: usize, columns: usize) -> MatchConstraints {
    let mut review = MatchConstraints::default();
    match (case / 10) % 6 {
        1 if rows > 0 && columns > 0 => {
            review
                .confirmed
                .push(FieldPair::new("s000", format!("t{:03}", columns - 1)));
            if rows > 1 {
                review.unmatched_sources.push("s001".into());
            }
        }
        2 => {
            review.unmatched_sources = (0..rows).map(|row| format!("s{row:03}").into()).collect();
        }
        3 => {
            for row in 0..rows {
                for column in (0..columns).step_by(2) {
                    review.forbidden.push(FieldPair::new(
                        format!("s{row:03}"),
                        format!("t{column:03}"),
                    ));
                }
            }
        }
        4 => {
            review.confirmed = (0..rows.min(columns))
                .map(|index| FieldPair::new(format!("s{index:03}"), format!("t{index:03}")))
                .collect();
        }
        5 => {
            for row in 0..rows {
                if row % 2 == 0 && row < columns {
                    review
                        .confirmed
                        .push(FieldPair::new(format!("s{row:03}"), format!("t{row:03}")));
                } else if row % 2 == 1 {
                    review.unmatched_sources.push(format!("s{row:03}").into());
                }
            }
        }
        _ => {}
    }
    review
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let check_only = match args.as_slice() {
        [] => false,
        [option] if option == "--check-only" => true,
        _ => return Err("expected no arguments or --check-only".into()),
    };
    let mut output = BufWriter::new(io::stdout().lock());
    let mut rng = Generator(SEED);
    writeln!(
        output,
        "fieldkin-assignment-compatibility-v1 cases={CASES} seed={SEED}"
    )?;
    for case in 0..CASES {
        let rows = rng.index(9);
        let columns = rng.index(9);
        let matrix = scores(case, rows, columns, &mut rng);
        let (rows, columns, matrix) = if case == 1 {
            (5, 3, fractional_dead_row_fixture())
        } else {
            (rows, columns, matrix)
        };
        let pairs = rows * columns;
        let work_per_solve = rows * rows * (rows + columns);
        let mut source = schema("s", rows);
        let mut target = schema("t", columns);
        let mut review = constraints(case, rows, columns);
        let mut config = Config {
            min_score: [0.0, 0.25, 0.5, 0.75][(case / 7) % 4],
            ambiguity_margin: [0.0, f64::EPSILON, 0.08][(case / 11) % 3],
            max_candidates: 1 + case % 4,
            one_to_one: case % 7 != 0,
            abstain_on_ambiguity: (case / 13) % 2 == 0,
            global_diagnostics: GlobalDiagnosticsConfig {
                max_solves: [0, 1, 16][(case / 3) % 3],
                max_work: match (case / 5) % 4 {
                    0 => work_per_solve.saturating_sub(1),
                    1 => work_per_solve,
                    2 => work_per_solve * 2,
                    _ => 8_388_608,
                },
                objective_margin: [0.0, 0.08, 8.0][case % 3],
            },
            ..Config::default()
        };
        if case == 1 {
            config.min_score = 0.0;
            config.abstain_on_ambiguity = false;
            config.global_diagnostics.max_solves = 16;
            config.global_diagnostics.max_work = 8_388_608;
        }
        let expected_error = if pairs > 0 && case % 41 == 0 {
            config.limits.max_pairs = pairs - 1;
            Some((MatchError::BudgetExceeded(BudgetKind::Pairs), 0))
        } else if pairs > 0 && case % 43 == 0 {
            config.limits.max_explanation_bytes = 0;
            Some((MatchError::BudgetExceeded(BudgetKind::ExplanationBytes), 1))
        } else {
            None
        };
        let trace: Trace = Arc::new(Mutex::new(Vec::new()));
        let engine = MatchEngine::with_matchers(
            config.clone(),
            vec![WeightedMatcher::new(
                1.0,
                MatrixMatcher {
                    scores: matrix.clone(),
                    trace: trace.clone(),
                },
            )],
        )?;
        let result = engine.match_schemas_with_constraints(&source, &target, &review);
        let calls = trace.lock().map_err(|_| "Trace mutex poisoned")?.clone();
        let expected_calls = if let Some((error, calls)) = &expected_error {
            assert_eq!(result.as_ref().unwrap_err(), error, "case {case}");
            *calls
        } else {
            assert!(result.is_ok(), "case {case}: {result:?}");
            pairs
        };
        if case == 1 {
            let selected: Vec<_> = result
                .as_ref()
                .expect("fixture result was checked above")
                .fields
                .iter()
                .map(|field| {
                    field
                        .selected
                        .as_ref()
                        .map(|candidate| candidate.target.0.as_str())
                })
                .collect();
            assert_eq!(
                selected,
                vec![None, Some("t000"), Some("t001"), Some("t002"), None],
                "fractional dead-row tie convention"
            );
        }
        let expected_trace: Vec<_> = (0..rows)
            .flat_map(|row| {
                (0..columns).map(move |column| (format!("s{row:03}"), format!("t{column:03}")))
            })
            .take(expected_calls)
            .collect();
        assert_eq!(calls, expected_trace, "callback order for case {case}");

        // Complete report equality also checks stable-ID ordering of witnesses,
        // competitions, candidates and the unmatched complements.
        trace.lock().map_err(|_| "Trace mutex poisoned")?.clear();
        source.fields.reverse();
        target.fields.reverse();
        review.confirmed.reverse();
        review.forbidden.reverse();
        review.unmatched_sources.reverse();
        let reordered = engine.match_schemas_with_constraints(&source, &target, &review);
        assert_eq!(result, reordered, "reordering for case {case}");
        assert_eq!(*trace.lock().map_err(|_| "Trace mutex poisoned")?, calls);

        if !check_only {
            writeln!(output, "case={case} rows={rows} columns={columns}")?;
            writeln!(
                output,
                "config={config:?} review={review:?} matrix={matrix:?}"
            )?;
            writeln!(output, "callbacks={calls:?}")?;
            writeln!(output, "result={result:?}")?;
        }
    }
    writeln!(
        output,
        "PASS {CASES} cases; original and reversed inputs; complete reports and callback traces"
    )?;
    output.flush()?;
    Ok(())
}
