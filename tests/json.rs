#![cfg_attr(not(feature = "json"), allow(missing_docs))]
#![cfg(feature = "json")]
//! The JSON boundary preserves review intent without manufacturing evidence.

use fieldkin::json::*;
use fieldkin::*;
use proptest::prelude::*;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const PRIVATE: &str = "private-sample-marker";
fn context() -> ReviewContext<'static> {
    ReviewContext {
        source_revision: "source-v1",
        target_revision: "target-v1",
    }
}
fn schema(prefix: &str, count: usize) -> Schema {
    Schema::new(
        (0..count)
            .map(|i| {
                Field::new(format!("{prefix}{i}"), "same", DataType::Text)
                    .with_samples(vec![SampleValue::Text(PRIVATE.into())])
            })
            .collect(),
    )
}
struct Matrix {
    values: Vec<Vec<Option<f64>>>,
    calls: Arc<AtomicUsize>,
}
impl Matcher for Matrix {
    fn name(&self) -> &str {
        PRIVATE
    }
    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(Evidence {
            score: self.values[source.id.0[1..].parse::<usize>().unwrap()]
                [target.id.0[1..].parse::<usize>().unwrap()],
            explanation: PRIVATE.into(),
        })
    }
}
fn engine(values: Vec<Vec<Option<f64>>>, config: Config) -> (MatchEngine, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    (
        MatchEngine::with_matchers(
            config,
            vec![WeightedMatcher::new(
                1.0,
                Matrix {
                    values,
                    calls: calls.clone(),
                },
            )],
        )
        .unwrap(),
        calls,
    )
}
fn report() -> MatchReport {
    engine(
        vec![vec![Some(0.9), Some(0.8)], vec![Some(0.85), None]],
        Config {
            min_score: 0.1,
            ambiguity_margin: 0.0,
            max_candidates: 1,
            one_to_one: true,
            abstain_on_ambiguity: false,
            global_diagnostics: GlobalDiagnosticsConfig {
                max_solves: 8,
                objective_margin: 2.0,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .0
    .match_schemas(&schema("s", 2), &schema("t", 2))
    .unwrap()
}
fn document() -> Vec<u8> {
    review_to_json(
        &MatchConstraints::default(),
        context(),
        &JsonLimits::default(),
    )
    .unwrap()
}
fn value(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

#[test]
fn report_keeps_selection_outside_top_k_and_every_global_witness() {
    let report = report();
    let exported = value(&report_to_json(&report, &ReportJsonOptions::default()).unwrap());
    assert_eq!(exported["format"], "fieldkin.report");
    assert_eq!(exported["version"], 1);
    assert_eq!(
        exported["report"]["fields"][0]["candidates"][0]["target"],
        "t0"
    );
    assert_eq!(exported["report"]["fields"][0]["selected"]["target"], "t1");
    assert!(exported["report"]["fields"][0]["diagnostics"]
        .as_array()
        .unwrap()
        .contains(&json!({"code":"displaced","target":"t0"})));
    let diagnostics = &exported["report"]["assignment_diagnostics"];
    assert_eq!(diagnostics["status"], "complete");
    assert_eq!(
        diagnostics["solves_used"],
        report.assignment_diagnostics.solves_used
    );
    assert_eq!(
        diagnostics["work_used"],
        report.assignment_diagnostics.work_used
    );
    assert_eq!(
        diagnostics["alternatives"].as_array().unwrap().len(),
        report.assignment_diagnostics.alternatives.len()
    );
    for (wire, witness) in diagnostics["alternatives"]
        .as_array()
        .unwrap()
        .iter()
        .zip(&report.assignment_diagnostics.alternatives)
    {
        assert_eq!(wire["objective"].as_f64().unwrap(), witness.objective);
        assert_eq!(wire["gap"].as_f64().unwrap(), witness.gap);
        for (wire, change) in wire["changes"]
            .as_array()
            .unwrap()
            .iter()
            .zip(&witness.changes)
        {
            assert_eq!(wire["source"], change.source.0);
            assert_eq!(
                wire["selected_target"],
                json!(change.selected_target.as_ref().map(|id| &id.0))
            );
            assert_eq!(
                wire["alternative_target"],
                json!(change.alternative_target.as_ref().map(|id| &id.0))
            );
        }
    }
}

#[test]
fn default_report_omits_all_free_form_extension_text() {
    let mut report = report();
    report.fields[0].candidates[0].warnings.push(PRIVATE.into());
    let bytes = report_to_json(&report, &ReportJsonOptions::default()).unwrap();
    assert!(!String::from_utf8(bytes.clone()).unwrap().contains(PRIVATE));
    let wire = value(&bytes);
    assert_eq!(wire["text_included"], false);
    assert!(wire["report"]["fields"][0]["candidates"][0]
        .get("warnings")
        .is_none());
    assert!(wire["report"]["fields"][0]["candidates"][0]["signals"][0]
        .get("name")
        .is_none());
    assert!(wire["report"]["fields"][0]["candidates"][0]["signals"][0]
        .get("explanation")
        .is_none());
    let explicit = report_to_json(
        &report,
        &ReportJsonOptions {
            include_text: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(String::from_utf8(explicit).unwrap().contains(PRIVATE));
}

#[test]
fn explicit_zero_score_confirmation_survives_export_and_resume() {
    let (engine, _) = engine(vec![vec![Some(0.0)]], Config::default());
    let review = MatchConstraints {
        confirmed: vec![FieldPair::new("s0", "t0")],
        ..Default::default()
    };
    let restored = review_from_json(
        &review_to_json(&review, context(), &JsonLimits::default()).unwrap(),
        context(),
        &JsonLimits::default(),
    )
    .unwrap();
    let report = engine
        .match_schemas_with_constraints(&schema("s", 1), &schema("t", 1), &restored)
        .unwrap();
    let wire = value(&report_to_json(&report, &ReportJsonOptions::default()).unwrap());
    assert_eq!(wire["report"]["fields"][0]["decision"], "confirmed");
    assert_eq!(wire["report"]["fields"][0]["selected"]["score"], 0.0);
    assert_eq!(wire["report"]["fields"][0]["selected"]["eligible"], false);
    assert_eq!(wire["report"]["unmatched_sources"], json!([]));
}

#[test]
fn ambiguity_unmatched_and_incomplete_diagnostics_remain_explicit() {
    let mut config = Config {
        one_to_one: true,
        ..Default::default()
    };
    let ambiguous = engine(vec![vec![Some(0.9); 2]; 2], config.clone())
        .0
        .match_schemas(&schema("s", 2), &schema("t", 2))
        .unwrap();
    let wire = value(&report_to_json(&ambiguous, &ReportJsonOptions::default()).unwrap());
    assert_eq!(wire["report"]["fields"][0]["decision"], "ambiguous");
    assert_eq!(
        wire["report"]["fields"][0]["alternatives"],
        json!(["t0", "t1"])
    );
    assert_eq!(wire["report"]["unmatched_sources"], json!(["s0", "s1"]));
    assert_eq!(wire["report"]["unmatched_targets"], json!(["t0", "t1"]));
    config.abstain_on_ambiguity = false;
    config.global_diagnostics.max_solves = 1;
    let incomplete = engine(vec![vec![Some(0.9); 2]; 2], config)
        .0
        .match_schemas(&schema("s", 2), &schema("t", 2))
        .unwrap();
    let wire = value(&report_to_json(&incomplete, &ReportJsonOptions::default()).unwrap());
    assert_eq!(
        wire["report"]["assignment_diagnostics"]["status"],
        "budget_exhausted"
    );
    assert_eq!(
        wire["report"]["target_competition"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn report_numbers_are_rejected_before_json_can_silently_replace_them() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.1, 1.1] {
        for slot in 0..4 {
            let mut report = report();
            match slot {
                0 => report.fields[0].candidates[0].score = invalid,
                1 => report.fields[0].selected.as_mut().unwrap().score = invalid,
                2 => report.fields[0].candidates[0].signals[0].weight = invalid,
                _ => report.fields[0].candidates[0].signals[0].evidence.score = Some(invalid),
            }
            assert_eq!(
                report_to_json(&report, &ReportJsonOptions::default()).unwrap_err(),
                JsonError::InvalidReport
            );
        }
    }
    for invalid in [f64::NAN, f64::INFINITY, -1.0] {
        for slot in 0..3 {
            let mut report = report();
            match slot {
                0 => report.assignment_diagnostics.base_objective = Some(invalid),
                1 => report.assignment_diagnostics.alternatives[0].objective = invalid,
                _ => report.assignment_diagnostics.alternatives[0].gap = invalid,
            }
            assert_eq!(
                report_to_json(&report, &ReportJsonOptions::default()).unwrap_err(),
                JsonError::InvalidReport
            );
        }
    }
}

#[test]
fn encoded_byte_limit_is_exact_and_returns_no_partial_document() {
    let report = report();
    let bytes = report_to_json(&report, &ReportJsonOptions::default()).unwrap();
    let mut options = ReportJsonOptions::default();
    options.limits.max_bytes = bytes.len();
    assert_eq!(report_to_json(&report, &options).unwrap(), bytes);
    options.limits.max_bytes -= 1;
    assert_eq!(
        report_to_json(&report, &options),
        Err(JsonError::LimitExceeded(JsonLimit::Bytes))
    );
    let review = MatchConstraints::default();
    let bytes = document();
    let mut limits = JsonLimits {
        max_bytes: bytes.len(),
        ..Default::default()
    };
    assert_eq!(review_to_json(&review, context(), &limits).unwrap(), bytes);
    limits.max_bytes -= 1;
    assert_eq!(
        review_to_json(&review, context(), &limits),
        Err(JsonError::LimitExceeded(JsonLimit::Bytes))
    );
    assert_eq!(
        review_from_json(&bytes, context(), &limits),
        Err(JsonError::LimitExceeded(JsonLimit::Bytes))
    );
}

#[test]
fn item_and_utf8_string_limits_cover_review_and_report() {
    let limits = JsonLimits {
        max_items: 1,
        ..Default::default()
    };
    assert_eq!(
        review_from_json(&document(), context(), &limits),
        Err(JsonError::LimitExceeded(JsonLimit::Items))
    );
    assert_eq!(
        report_to_json(
            &report(),
            &ReportJsonOptions {
                limits,
                ..Default::default()
            }
        ),
        Err(JsonError::LimitExceeded(JsonLimit::Items))
    );
    let limits = JsonLimits {
        max_string_bytes: 1,
        ..Default::default()
    };
    assert_eq!(
        review_from_json(&document(), context(), &limits),
        Err(JsonError::LimitExceeded(JsonLimit::StringBytes))
    );
    let mut report = report();
    report.fields[0].source = "é".into();
    assert_eq!(
        report_to_json(
            &report,
            &ReportJsonOptions {
                limits,
                ..Default::default()
            }
        ),
        Err(JsonError::LimitExceeded(JsonLimit::StringBytes))
    );
}

#[test]
fn review_retains_order_unicode_escapes_and_identical_duplicates() {
    let pair = FieldPair::new("源\"\\\n", "目標\t");
    let review = MatchConstraints {
        confirmed: vec![pair.clone(), pair],
        forbidden: vec![FieldPair::new("two", "wrong")],
        unmatched_sources: vec!["three".into(), "three".into()],
    };
    let context = ReviewContext {
        source_revision: "源-\"v1",
        target_revision: "目标-v2",
    };
    let bytes = review_to_json(&review, context, &JsonLimits::default()).unwrap();
    assert_eq!(
        review_from_json(&bytes, context, &JsonLimits::default()).unwrap(),
        review
    );
    assert_eq!(
        review_to_json(&review, context, &JsonLimits::default()).unwrap(),
        bytes
    );
}

#[test]
fn matching_still_counts_raw_repeated_directives() {
    let review = MatchConstraints {
        confirmed: vec![FieldPair::new("s0", "t0"); 2],
        ..Default::default()
    };
    let parsed = review_from_json(
        &review_to_json(&review, context(), &JsonLimits::default()).unwrap(),
        context(),
        &JsonLimits::default(),
    )
    .unwrap();
    let mut config = Config::default();
    config.limits.max_fields = 1;
    let (engine, calls) = engine(vec![vec![Some(0.9)]], config);
    assert_eq!(
        engine.match_schemas_with_constraints(&schema("s", 1), &schema("t", 1), &parsed),
        Err(MatchError::BudgetExceeded(BudgetKind::Constraints))
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn stale_context_is_rejected_before_application_calls_matchers() {
    let (engine, calls) = engine(vec![vec![Some(0.9)]], Config::default());
    let stale = ReviewContext {
        source_revision: PRIVATE,
        ..context()
    };
    let result = review_from_json(&document(), stale, &JsonLimits::default()).map(|review| {
        engine.match_schemas_with_constraints(&schema("s", 1), &schema("t", 1), &review)
    });
    assert_eq!(result.unwrap_err(), JsonError::ContextMismatch);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn resume_reordered_schemas_produces_identical_complete_report_and_export() {
    let (engine, _) = engine(
        vec![vec![Some(0.9); 2]; 2],
        Config {
            one_to_one: true,
            ..Default::default()
        },
    );
    let review = MatchConstraints {
        confirmed: vec![FieldPair::new("s0", "t1")],
        unmatched_sources: vec!["s1".into()],
        ..Default::default()
    };
    let mut source = schema("s", 2);
    let mut target = schema("t", 2);
    let before = engine
        .match_schemas_with_constraints(&source, &target, &review)
        .unwrap();
    let parsed = review_from_json(
        &review_to_json(&review, context(), &JsonLimits::default()).unwrap(),
        context(),
        &JsonLimits::default(),
    )
    .unwrap();
    source.fields.reverse();
    target.fields.reverse();
    let after = engine
        .match_schemas_with_constraints(&source, &target, &parsed)
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        report_to_json(&before, &Default::default()),
        report_to_json(&after, &Default::default())
    );
}

#[test]
fn malformed_unknown_duplicate_and_deep_documents_have_private_errors() {
    let base = String::from_utf8(document()).unwrap();
    let mut unknown = value(base.as_bytes());
    unknown[PRIVATE] = json!(PRIVATE);
    let malformed = [
        PRIVATE.to_string(),
        format!("{}{}", base, base),
        base.replace("\"version\":1", "\"version\":1,\"version\":1"),
        serde_json::to_string(&unknown).unwrap(),
        format!("{}\"{}\"{}", "[".repeat(256), PRIVATE, "]".repeat(256)),
    ];
    for text in malformed {
        let error =
            review_from_json(text.as_bytes(), context(), &JsonLimits::default()).unwrap_err();
        assert_eq!(error, JsonError::InvalidDocument);
        assert!(!format!("{error:?} {error}").contains(PRIVATE));
    }
    let mut nested = value(base.as_bytes());
    nested["context"][PRIVATE] = json!(PRIVATE);
    assert_eq!(
        review_from_json(
            &serde_json::to_vec(&nested).unwrap(),
            context(),
            &JsonLimits::default()
        ),
        Err(JsonError::InvalidDocument)
    );
}

#[test]
fn formats_versions_and_empty_revisions_are_explicit_failures() {
    let mut doc = value(&document());
    doc["format"] = json!(PRIVATE);
    assert_eq!(
        review_from_json(
            &serde_json::to_vec(&doc).unwrap(),
            context(),
            &JsonLimits::default()
        ),
        Err(JsonError::UnsupportedFormat)
    );
    doc["format"] = json!("fieldkin.review");
    doc["version"] = json!(2);
    assert_eq!(
        review_from_json(
            &serde_json::to_vec(&doc).unwrap(),
            context(),
            &JsonLimits::default()
        ),
        Err(JsonError::UnsupportedVersion)
    );
    assert_eq!(
        review_from_json(
            &document(),
            ReviewContext {
                source_revision: "",
                ..context()
            },
            &JsonLimits::default()
        ),
        Err(JsonError::InvalidReview)
    );
    let bytes = report_to_json(&report(), &ReportJsonOptions::default()).unwrap();
    assert_eq!(
        review_from_json(&bytes, context(), &JsonLimits::default()),
        Err(JsonError::InvalidDocument)
    );
}

#[test]
fn engine_remains_authority_for_unknown_and_conflicting_review_ids() {
    let (engine, calls) = engine(vec![vec![Some(0.9)]], Config::default());
    for review in [
        MatchConstraints {
            confirmed: vec![FieldPair::new("unknown", "t0")],
            ..Default::default()
        },
        MatchConstraints {
            confirmed: vec![FieldPair::new("s0", "t0")],
            forbidden: vec![FieldPair::new("s0", "t0")],
            ..Default::default()
        },
    ] {
        let restored = review_from_json(
            &review_to_json(&review, context(), &JsonLimits::default()).unwrap(),
            context(),
            &JsonLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            engine.match_schemas_with_constraints(&schema("s", 1), &schema("t", 1), &restored),
            Err(MatchError::InvalidConstraints(_))
        ));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn structured_semantic_issues_preserve_axis_without_private_labels() {
    let source = Schema::new(vec![Field::new("s", "price", DataType::Decimal)
        .with_hints(SemanticHints {
            currency: Some(PRIVATE.into()),
            ..Default::default()
        })]);
    let target = Schema::new(vec![Field::new("t", "price", DataType::Decimal)
        .with_hints(SemanticHints {
            currency: Some("other".into()),
            ..Default::default()
        })]);
    let report = MatchEngine::new(Config::default())
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap();
    let bytes = report_to_json(&report, &ReportJsonOptions::default()).unwrap();
    assert!(!String::from_utf8(bytes.clone()).unwrap().contains(PRIVATE));
    assert!(
        value(&bytes)["report"]["fields"][0]["candidates"][0]["issues"]
            .as_array()
            .unwrap()
            .contains(&json!({"code":"semantic_conflict","axis":"currency"}))
    );
}

#[test]
fn empty_ids_and_nested_duplicate_keys_are_rejected() {
    let empty = MatchConstraints {
        confirmed: vec![FieldPair::new("", "t0")],
        ..Default::default()
    };
    assert_eq!(
        review_to_json(&empty, context(), &JsonLimits::default()),
        Err(JsonError::InvalidReview)
    );
    let mut report = report();
    report.unmatched_targets.push("".into());
    assert_eq!(
        report_to_json(&report, &ReportJsonOptions::default()),
        Err(JsonError::InvalidReport)
    );
    let review = MatchConstraints {
        confirmed: vec![FieldPair::new("s0", "t0")],
        ..Default::default()
    };
    let text =
        String::from_utf8(review_to_json(&review, context(), &JsonLimits::default()).unwrap())
            .unwrap();
    for text in [
        text.replace("\"source\":\"s0\"", "\"source\":\"s0\",\"source\":\"s0\""),
        text.replace(
            "\"source_revision\":\"source-v1\"",
            "\"source_revision\":\"source-v1\",\"source_revision\":\"source-v1\"",
        ),
    ] {
        assert_eq!(
            review_from_json(text.as_bytes(), context(), &JsonLimits::default()),
            Err(JsonError::InvalidDocument)
        );
    }
    assert_eq!(
        review_from_json(&[0xff, 0xfe], context(), &JsonLimits::default()),
        Err(JsonError::InvalidDocument)
    );
}

#[test]
fn omitted_text_cannot_exhaust_export_string_limits() {
    let mut report = report();
    report.fields[0].candidates[0].signals[0].name = PRIVATE.repeat(300);
    report.fields[0].candidates[0].signals[0]
        .evidence
        .explanation = PRIVATE.repeat(300);
    report.fields[0].candidates[0].warnings = vec![PRIVATE.repeat(300)];
    assert!(report_to_json(&report, &ReportJsonOptions::default()).is_ok());
    assert_eq!(
        report_to_json(
            &report,
            &ReportJsonOptions {
                include_text: true,
                ..Default::default()
            }
        ),
        Err(JsonError::LimitExceeded(JsonLimit::StringBytes))
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn arbitrary_bounded_bytes_fail_safely(bytes in proptest::collection::vec(any::<u8>(),0..512)) {
        let limits = JsonLimits { max_bytes: 256, ..Default::default() };
        let result = review_from_json(&bytes, context(), &limits);
        if bytes.len() > 256 { prop_assert_eq!(result, Err(JsonError::LimitExceeded(JsonLimit::Bytes))); }
        else if let Err(error) = result { prop_assert!(error.to_string().len() < 80); }
    }

    #[test]
    fn arbitrary_unicode_review_ids_round_trip(source in "(?s).{1,32}", target in "(?s).{1,32}") {
        let review = MatchConstraints { confirmed: vec![FieldPair::new(source,target)], ..Default::default() };
        let bytes = review_to_json(&review, context(), &JsonLimits::default()).unwrap();
        prop_assert_eq!(review_from_json(&bytes, context(), &JsonLimits::default()).unwrap(),review);
    }
}
