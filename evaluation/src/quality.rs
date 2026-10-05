//! Frozen fresh qualification. This route is absent from ordinary CI snapshots.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use fieldkin::{Config, Decision, MatchEngine, NameMatcher, WeightedMatcher};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::corpus;
use crate::metrics::{score_case, Counts};
use crate::model::{Family, LabelKind};

pub(crate) const MODELS: [&str; 4] = [
    "default",
    "name_only_070",
    "name_only_exact",
    "contextual_quality",
];
pub(crate) const VARIANTS: [&str; 5] = [
    "base",
    "reordered",
    "without_samples",
    "null_heavy",
    "separator_noise",
];

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn configuration_json(config: &Config) -> Value {
    let contextual = config.contextual_evidence.map(|evidence| {
        json!({
            "strict_identifier_samples":evidence.strict_identifier_samples,
            "scoped_support":evidence.scoped_support,
            "distinguish_relationships":evidence.distinguish_relationships,
            "independent_sample_populations":evidence.independent_sample_populations,
        "preserve_score_ranking":evidence.preserve_score_ranking
        ,"identifier_word_forms":evidence.identifier_word_forms
        })
    });
    let sample = fieldkin::SampleMatcher::default();
    let limits = &config.limits;
    json!({"min_score":config.min_score,"ambiguity_margin":config.ambiguity_margin,"top_k":config.max_candidates,
        "weights":{"name":0.65,"type":0.20,"sample":0.15},"contextual_evidence":contextual,
        "abstain_on_ambiguity":config.abstain_on_ambiguity,"reject_incompatible_types":config.reject_incompatible_types,
        "corroboration":config.corroboration.map(|value|json!({"min_name_score":value.min_name_score,"min_sample_score":value.min_sample_score})),
        "built_in_signals":{"name_aliases":NameMatcher::default().aliases,"sample_min_non_null":sample.min_non_null,"sample_reliability":format!("{:?}",sample.reliability)},
        "limits":{"max_fields":limits.max_fields,"max_pairs":limits.max_pairs,"max_signal_evaluations":limits.max_signal_evaluations,"max_explanation_bytes":limits.max_explanation_bytes,"max_name_bytes":limits.max_name_bytes,"max_samples_per_field":limits.max_samples_per_field,"max_sample_bytes":limits.max_sample_bytes,"max_total_sample_bytes":limits.max_total_sample_bytes},
        "global_diagnostics":{"max_solves":config.global_diagnostics.max_solves,"max_work":config.global_diagnostics.max_work,"objective_margin":config.global_diagnostics.objective_margin},
        "sample_population_assumption":if config.contextual_evidence.is_some_and(|evidence|evidence.independent_sample_populations) {"independent populations; disjoint sample values are inconclusive when informative lexical and representation support agree"} else {"overlapping samples required by configured contextual support"},
        "name_conflicts":config.name_conflicts.iter().map(|rule|json!({"kind":format!("{:?}",rule.kind),"alternatives":rule.alternatives})).collect::<Vec<_>>()})
}

pub(crate) fn candidate_configuration() -> Value {
    configuration_json(&Config::contextual_quality())
}

pub fn print_configuration() {
    println!(
        "{}",
        serde_json::to_string_pretty(&candidate_configuration()).unwrap()
    );
}

pub fn print_development_identity() -> Result<(), String> {
    let corpora = crate::readiness::cases()?;
    let observations = corpora
        .iter()
        .map(|(name, cases)| {
            Ok((
                name.clone(),
                crate::readiness::observable_inputs_sha256(cases)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let scorer = corpora
        .iter()
        .map(|(name, cases)| {
            Ok((
                name.clone(),
                crate::readiness_regression::scorer_fingerprint(cases)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    println!(
        "{}",
        serde_json::to_string(
            &json!({"observable_inputs_sha256":observations,"scorer_inputs_sha256":scorer})
        )
        .map_err(|error| error.to_string())?
    );
    Ok(())
}

fn engine(model: &str, assignment: &str) -> Result<MatchEngine, String> {
    let mut config = if model == "contextual_quality" {
        Config::contextual_quality()
    } else {
        Config::default()
    };
    config.one_to_one = assignment == "one_to_one";
    if model == "name_only_exact" {
        config.min_score = 1.0;
    }
    if model.starts_with("name_only") {
        MatchEngine::with_matchers(
            config,
            vec![WeightedMatcher::new(1.0, NameMatcher::default())],
        )
    } else {
        MatchEngine::new(config)
    }
    .map_err(|error| error.to_string())
}

fn quality(counts: &Counts) -> bool {
    counts.proposals > 0
        && counts.unique_fields > 0
        && counts.candidate_relevant > 0
        && (counts.correct_proposals as u128) * 100 >= (counts.proposals as u128) * 95
        && (counts.unique_proposals as u128) * 100 >= (counts.unique_fields as u128) * 60
        && (counts.candidate_hits as u128) * 100 >= (counts.candidate_relevant as u128) * 90
}

fn write_new(path: &Path, text: &str) -> Result<(), String> {
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| error.to_string())?
        .write_all(text.as_bytes())
        .map_err(|error| error.to_string())
}

pub fn run(args: Vec<String>) -> Result<(), String> {
    let mut args = args.into_iter();
    let mut options = BTreeMap::new();
    let mut authorized = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--acknowledge-new-holdout" if !authorized => authorized = true,
            "--protocol"
            | "--corpus"
            | "--provenance"
            | "--output"
            | "--frozen-binary-sha256"
            | "--frozen-build-record-sha256" => {
                if options
                    .insert(
                        arg,
                        args.next()
                            .ok_or("missing frozen qualification option value")?,
                    )
                    .is_some()
                {
                    return Err("duplicate qualification option".into());
                }
            }
            _ => return Err("qualification accepts only authorized frozen-run options".into()),
        }
    }
    if !authorized || options.len() != 6 {
        return Err("run through quality-qualify after quality-freeze; all frozen identities and authorization are required".into());
    }
    let bytes = |flag: &str| -> Result<Vec<u8>, String> {
        fs::read(options.get(flag).ok_or("missing frozen option")?)
            .map_err(|error| error.to_string())
    };
    let protocol_bytes = bytes("--protocol")?;
    let corpus_bytes = bytes("--corpus")?;
    let provenance_bytes = bytes("--provenance")?;
    let protocol: Value =
        serde_json::from_slice(&protocol_bytes).map_err(|error| error.to_string())?;
    if protocol["version"] != "fieldkin-quality-v1"
        || protocol["candidate"] != "contextual_quality"
        || protocol["configuration"] != candidate_configuration()
        || protocol["models"] != json!(MODELS)
        || protocol["assignments"] != json!(["independent", "one_to_one"])
        || protocol["variants"] != json!(VARIANTS)
        || protocol["quality_targets"]
            != json!({"precision":0.95,"unique_coverage":0.60,"candidate_recall_at_5":0.90,"nonzero_proposals":true})
    {
        return Err(
            "protocol differs from the frozen public candidate or required quality contract".into(),
        );
    }
    let binary = fs::read(std::env::current_exe().map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    if options.get("--frozen-binary-sha256") != Some(&digest(&binary)) {
        return Err("running executable differs from frozen binary".into());
    }
    let output = PathBuf::from(options.get("--output").ok_or("missing output")?);
    if output.exists() {
        return Err("fresh qualification output is required".into());
    }
    let mut families: Vec<Family> =
        serde_json::from_slice(&corpus_bytes).map_err(|error| error.to_string())?;
    if families.len() < 12 {
        return Err("fresh qualification needs at least twelve families".into());
    }
    let mut ids = BTreeSet::new();
    let mut domains = BTreeMap::<String, usize>::new();
    let mut kinds = BTreeSet::new();
    for family in &families {
        corpus::validate_family(family)?;
        if !ids.insert(family.id.clone()) {
            return Err("duplicate fresh family".into());
        }
        *domains.entry(family.domain.clone()).or_default() += 1;
        for label in &family.labels {
            kinds.insert(match label.kind {
                LabelKind::Match => "match",
                LabelKind::NoMatch => "no_match",
                LabelKind::Ambiguous => "ambiguous",
            });
        }
    }
    let required_domains = [
        "product_import",
        "financial_records",
        "crm_contacts",
        "operational_telemetry",
    ];
    if domains.len() != 4
        || required_domains
            .iter()
            .any(|domain| domains.get(*domain).copied().unwrap_or(0) < 3)
        || kinds != ["match", "no_match", "ambiguous"].into_iter().collect()
    {
        return Err("fresh qualification requires four domains with three families each and all label kinds".into());
    }
    families.sort_by(|left, right| left.id.cmp(&right.id));
    let cases = corpus::cases(&families)?;
    let decisions = cases.iter().map(|case| case.labels.len()).sum::<usize>();
    if cases.len() < 60
        || decisions < 360
        || protocol["inventory"]
            != json!({"families":families.len(),"cases":cases.len(),"decisions":decisions})
    {
        return Err("fresh inventory differs from frozen protocol minimums or counts".into());
    }
    let mut rows = Vec::new();
    let mut predictions = String::new();
    for model in MODELS {
        for assignment in ["independent", "one_to_one"] {
            let engine = engine(model, assignment)?;
            let mut total = Counts::default();
            let mut by_family = BTreeMap::<String, Counts>::new();
            let mut by_domain = BTreeMap::<String, Counts>::new();
            let mut by_variant = BTreeMap::<String, Counts>::new();
            for case in &cases {
                // Neither labels nor evaluation-only metadata enter engine inputs.
                let source = corpus::schema(&case.source)?;
                let target = corpus::schema(&case.target)?;
                let report = engine
                    .match_schemas(&source, &target)
                    .map_err(|error| error.to_string())?;
                if report
                    .fields
                    .iter()
                    .any(|field| field.decision == Decision::Confirmed)
                {
                    return Err("caller-confirmed qualification is forbidden".into());
                }
                let counts = score_case(&case.labels, &report)?;
                total.add(&counts);
                by_family
                    .entry(case.family_id.clone())
                    .or_default()
                    .add(&counts);
                by_domain
                    .entry(case.domain.clone())
                    .or_default()
                    .add(&counts);
                by_variant
                    .entry(case.variant.clone())
                    .or_default()
                    .add(&counts);
                predictions.push_str(&serde_json::to_string(&json!({"case":case.id,"family":case.family_id,"domain":case.domain,"variant":case.variant,"model":model,"assignment":assignment,"counts":counts,
                    "fields":report.fields.iter().map(|field| {
                        let label=case.labels.iter().find(|label|label.source == field.source.0).expect("validated scorer label");
                        json!({"source":field.source.0,"decision":format!("{:?}",field.decision),"selected":field.selected.as_ref().map(|candidate|candidate.target.0.clone()),
                            "label_kind":match label.kind {LabelKind::Match=>"match",LabelKind::NoMatch=>"no_match",LabelKind::Ambiguous=>"ambiguous"},"gold_targets":label.targets,
                            "ranked_targets":field.candidates.iter().take(5).map(|candidate|candidate.target.0.clone()).collect::<Vec<_>>()})
                    }).collect::<Vec<_>>(),
                    "report_sha256":digest(format!("{report:?}").as_bytes())})).map_err(|error|error.to_string())?);
                predictions.push('\n');
            }
            rows.push(json!({"model":model,"assignment":assignment,"counts":total,"metrics":total.metrics(),"quality_targets_met":quality(&total),"by_family":by_family,"by_domain":by_domain,"by_variant":by_variant}));
        }
    }
    let qualified = rows
        .iter()
        .filter(|row| row["model"] == "contextual_quality")
        .all(|row| row["quality_targets_met"] == true);
    let report = json!({"metadata":{"protocol":protocol,"protocol_sha256":digest(&protocol_bytes),"corpus_sha256":digest(&corpus_bytes),"provenance_sha256":digest(&provenance_bytes),
        "frozen_binary_sha256":options["--frozen-binary-sha256"],"frozen_build_record_sha256":options["--frozen-build-record-sha256"],"configuration":candidate_configuration(),
        "caller_hints_used":false,"caller_confirmations_used":false,"new_reserved_holdout_scored":true,"other_reserved_holdouts_scored":false},
        "inventory":{"families":families.len(),"cases":cases.len(),"decisions":decisions,"partition":"all_holdout"},"rows":rows,"fixed_synthetic_quality_targets_met":qualified,"production_accuracy_certified":false});
    fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    write_new(
        &output.join("qualification.json"),
        &(serde_json::to_string_pretty(&report).map_err(|error| error.to_string())? + "\n"),
    )?;
    write_new(&output.join("predictions.jsonl"), &predictions)?;
    write_new(&output.join("qualification.md"), &format!("# Fresh synthetic qualification\n\nPublic candidate: `Config::contextual_quality()`. All families, five variants and both assignments included. Fixed targets met: **{qualified}**. This synthetic result does not certify production accuracy. Detailed counts and variant/domain slices are in qualification.json.\n"))?;
    println!("First fresh qualification recorded; fixed synthetic targets met: {qualified}");
    Ok(())
}
