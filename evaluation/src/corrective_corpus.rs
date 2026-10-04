//! Original synthetic corrective-cycle fixtures; no matcher is invoked here.
use crate::model::{Case, Family, InputField, Label, LabelKind};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub const SEED: u64 = 20_261_004;
pub const DOMAINS: [&str; 4] = [
    "fulfillment",
    "subscription_billing",
    "research_labs",
    "civic_records",
];
pub const VARIANTS: [&str; 3] = ["base", "reordered", "null_heavy"];
pub const HOLDOUT: [&str; 12] = [
    "fulfillment_02",
    "fulfillment_04",
    "fulfillment_06",
    "subscriptions_02",
    "subscriptions_04",
    "subscriptions_06",
    "laboratory_02",
    "laboratory_04",
    "laboratory_06",
    "civic_02",
    "civic_04",
    "civic_06",
];
pub const PROVENANCE: &str = "Copyright 2026 Fieldkin contributors; MIT OR Apache-2.0. Twenty-four independently specified, original synthetic business scenarios across four consumer domains. No public datasets, personal records, algorithm outputs or inferred labels are used. Match labels require the same fact in the same existing representation; no-match labels reject absent facts and transformations. Ambiguous labels require abstention. Concepts and rationales never enter matching.";
pub const PARTITION_RATIONALE: &str = "Before any scoring, odd-numbered families 01/03/05 in each domain are development; even-numbered families 02/04/06 are holdout. The rule gives three independently specified families per domain to each split without inspecting model performance. All three variants of a family remain together. Variants are correlated robustness checks, not independent semantic families. This public holdout is a process constraint, not hidden data.";
// Exact authored source is available for provenance hashing; include_str reads
// this file as text and does not recursively evaluate its contents.
pub const SOURCE_TEXT: &str = include_str!("corrective_corpus.rs");

fn field(id: &str, name: &str, kind: &str, concept: &str, samples: Value) -> InputField {
    InputField {
        id: id.into(),
        name: name.into(),
        data_type: kind.into(),
        concept: concept.into(),
        samples: match samples {
            Value::Null => None,
            Value::Array(values) => Some(values),
            value => Some(vec![value]),
        },
    }
}
fn label(source: &str, kind: LabelKind, targets: &[&str], rationale: &str) -> Label {
    Label {
        source: source.into(),
        kind,
        targets: targets.iter().map(|id| (*id).into()).collect(),
        rationale: rationale.into(),
    }
}

pub fn families() -> Result<Vec<Family>, String> {
    let mut families = vec![

Family {
id: "fulfillment_01".into(),
domain: "fulfillment".into(),
title: "Warehouse dispatch handoff".into(),
tags: vec!["renamed".into(),"ambiguous_date".into(),"misleading_status".into()],
rationale: "A warehouse exports packed consignments into a dispatch ledger. Packing workflow and delivery workflow differ; an undocumented date lacks an event definition. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","ConsignmentRef","Text","Packed consignment identity",json!(["CS-A11", "CS-A18", "CS-A24"])),
field("s2","UnitsPacked","Integer","Units packed in this consignment",json!([7, 11, 4])),
field("s3","PackFinished","Timestamp","Packing completion instant",json!(["2026-04-01T08:11:00Z", "2026-04-01T08:22:00Z", "2026-04-01T08:34:00Z"])),
field("s4","CarrierRef","Text","Carrier tracking identity",json!(["TR-C51", "TR-C62", "TR-C77"])),
field("s5","status","Text","Warehouse picking workflow status",json!(["ready", "held", "ready"])),
field("s6","date","Date","Undocumented consignment event date",json!([])),
],
target: vec![
field("t1","shipment_id","Text","Packed consignment identity",json!(["CS-A24", "CS-A11", "CS-A18"])),
field("t2","packed_quantity","Integer","Units packed in this consignment",json!([4, 7, 11])),
field("t3","packed_at","Timestamp","Packing completion instant",json!(["2026-04-01T08:34:00Z", "2026-04-01T08:11:00Z", "2026-04-01T08:22:00Z"])),
field("t4","tracking_number","Text","Carrier tracking identity",json!(["TR-C77", "TR-C51", "TR-C62"])),
field("t5","status","Text","Carrier delivery workflow status",json!(["ready", "held", "ready"])),
field("t6","dispatch_date","Date","Date the consignment left the depot",json!([])),
field("t7","arrival_date","Date","Date the consignment reached its recipient",json!([])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identifiers refer to the same packed consignment."),
label("s2",LabelKind::Match,&["t2"],"Both quantities count packed units without conversion."),
label("s3",LabelKind::Match,&["t3"],"Both timestamps record completion of packing."),
label("s4",LabelKind::Match,&["t4"],"Both references identify carrier tracking records."),
label("s5",LabelKind::NoMatch,&[],"Picking state is not delivery state, even when the observed labels coincide."),
label("s6",LabelKind::Ambiguous,&["t6","t7"],"The source does not specify whether its date records dispatch or arrival; both target events remain plausible."),
],},
Family {
id: "fulfillment_02".into(),
domain: "fulfillment".into(),
title: "Carrier checkpoint events".into(),
tags: vec!["disjoint_samples".into(),"units".into(),"generic_names".into()],
rationale: "A carrier event archive imports checkpoints from another regional shard. Field meanings agree for event facts despite disjoint example rows. Net goods mass has no gross parcel mass equivalent. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","tracking","Text","Carrier tracking identity",json!(["ZX-101", "ZX-102", "ZX-103"])),
field("s2","event_code","Text","Carrier checkpoint event category",json!(["DEP", "HUB", "DEL"])),
field("s3","location","Text","Checkpoint depot code",json!(["N-17", "N-28", "N-39"])),
field("s4","time","Timestamp","Checkpoint observation instant",json!(["2026-05-02T11:01:00Z", "2026-05-02T12:02:00Z", "2026-05-02T13:03:00Z"])),
field("s5","created_at","Timestamp","Invoice creation instant",json!(["2026-05-01T00:00:00Z", "2026-05-01T00:00:00Z", "2026-05-01T00:00:00Z"])),
field("s6","net_weight_kg","Float","Goods-only mass in kilograms",json!([2.5, 3.7, 4.1])),
],
target: vec![
field("t1","tracking_id","Text","Carrier tracking identity",json!(["ZX-201", "ZX-202", "ZX-203"])),
field("t2","checkpoint_type","Text","Carrier checkpoint event category",json!(["HUB", "DEL", "DEP"])),
field("t3","depot_code","Text","Checkpoint depot code",json!(["S-41", "S-52", "S-63"])),
field("t4","observed_at","Timestamp","Checkpoint observation instant",json!(["2026-05-03T09:01:00Z", "2026-05-03T10:02:00Z", "2026-05-03T11:03:00Z"])),
field("t5","created_at","Timestamp","Archive row ingestion instant",json!(["2026-05-01T00:00:00Z", "2026-05-01T00:00:00Z", "2026-05-01T00:00:00Z"])),
field("t6","gross_weight_kg","Float","Parcel mass including packaging in kilograms",json!([2.5, 3.7, 4.1])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"The same tracking namespace is sampled from disjoint regional rows."),
label("s2",LabelKind::Match,&["t2"],"Both fields describe checkpoint categories."),
label("s3",LabelKind::Match,&["t3"],"Both fields identify observation depots in one carrier code system."),
label("s4",LabelKind::Match,&["t4"],"Both record checkpoint observation instants, although the samples concern different events."),
label("s5",LabelKind::NoMatch,&[],"Invoice creation is unrelated to archive ingestion."),
label("s6",LabelKind::NoMatch,&[],"Goods-only mass differs from packaged mass; identical sample numbers do not remove that distinction."),
],},
Family {
id: "fulfillment_03".into(),
domain: "fulfillment".into(),
title: "Return authorization reconciliation".into(),
tags: vec!["duplicate_names".into(),"amount_ambiguity".into(),"misleading_date".into()],
rationale: "A returns team reconciles authorizations with intake. Two quantity columns retain separate approved and received meanings, while a refund total lacks documented tax scope. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","RMA","Text","Return authorization identity",json!(["RA-701", "RA-716", "RA-729"])),
field("s2","item","Text","Returned product identifier",json!(["IT-Q1", "IT-Q2", "IT-Q3"])),
field("s3","qty","Integer","Quantity approved for return",json!([5, 8, 4])),
field("s4","qty","Integer","Quantity physically received",json!([4, 7, 3])),
field("s5","date","Date","Date the customer requested a return",json!(["2026-06-02", "2026-06-04", "2026-06-08"])),
field("s6","refund","Decimal","Undocumented refund total before or after tax",json!(null)),
],
target: vec![
field("t1","return_id","Text","Return authorization identity",json!(["RA-729", "RA-701", "RA-716"])),
field("t2","sku","Text","Returned product identifier",json!(["IT-Q3", "IT-Q1", "IT-Q2"])),
field("t3","qty","Integer","Quantity physically received",json!([3, 4, 7])),
field("t4","qty","Integer","Quantity approved for return",json!([4, 5, 8])),
field("t5","date","Date","Date the return case was closed",json!(["2026-06-02", "2026-06-04", "2026-06-08"])),
field("t6","gross_refund","Decimal","Refund including tax",json!(null)),
field("t7","net_refund","Decimal","Refund excluding tax",json!(null)),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"RMA and return_id identify the same authorization."),
label("s2",LabelKind::Match,&["t2"],"Both fields identify the returned item."),
label("s3",LabelKind::Match,&["t4"],"Approved quantity remains distinct from received quantity despite duplicate headings."),
label("s4",LabelKind::Match,&["t3"],"This quantity counts physical receipts, not authorization limits."),
label("s5",LabelKind::NoMatch,&[],"Request and closure dates describe different lifecycle events."),
label("s6",LabelKind::Ambiguous,&["t6","t7"],"Neither metadata nor samples identify the source refund tax scope."),
],},
Family {
id: "fulfillment_04".into(),
domain: "fulfillment".into(),
title: "Pick-task execution logs".into(),
tags: vec!["generic_names".into(),"coincidental_ids".into(),"units".into(),"empty_samples".into()],
rationale: "A warehouse moves picker task logs into an execution table. Stable task and order namespaces are different even when both use small integer-like keys. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","id","Text","Picking task identity",json!(["11", "12", "13"])),
field("s2","col_1","Text","Source storage-bin code",json!(["A-04", "B-09", "C-12"])),
field("s3","priority","Integer","Picking task priority rank",json!([1, 3, 2])),
field("s4","status","Text","Picking execution state",json!(["open", "held", "open"])),
field("s5","weight","Float","Picked goods mass in grams",json!([500, 800, 1200])),
field("s6","operator_note","Text","Picker freeform task note",json!([])),
],
target: vec![
field("t1","task_id","Text","Picking task identity",json!(["13", "11", "12"])),
field("t2","id","Text","Customer order identity",json!(["11", "12", "13"])),
field("t3","bin_code","Text","Source storage-bin code",json!(["C-12", "A-04", "B-09"])),
field("t4","priority_rank","Integer","Picking task priority rank",json!([2, 1, 3])),
field("t5","status","Text","Quality-control inspection state",json!(["open", "held", "open"])),
field("t6","weight","Float","Picked goods mass in kilograms",json!([500, 800, 1200])),
field("t7","picker_comment","Text","Picker freeform task note",json!(null)),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Task identity belongs to task_id, not the coinciding order ID domain."),
label("s2",LabelKind::Match,&["t3"],"The undocumented heading contains the source storage-bin fact."),
label("s3",LabelKind::Match,&["t4"],"Both fields use the same task priority scale."),
label("s4",LabelKind::NoMatch,&[],"Execution and inspection states are separate workflow facts."),
label("s5",LabelKind::NoMatch,&[],"Mass values require a grams-to-kilograms conversion, outside direct matching."),
label("s6",LabelKind::Match,&["t7"],"Both fields store the same picker-authored note; absence of samples does not change the intended fact."),
],},
Family {
id: "fulfillment_05".into(),
domain: "fulfillment".into(),
title: "Container loading audit".into(),
tags: vec!["duplicate_source_facts".into(),"constant_samples".into(),"ambiguous_time".into()],
rationale: "A loading audit repeats the scanned package count in two export columns. The destination retains one count, while a timestamp is undocumented and could describe opening or sealing. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","container","Text","Shipping container identity",json!(["CN-A8", "CN-B9", "CN-C4"])),
field("s2","count","Integer","Scanned packages in the container",json!([17, 23, 31])),
field("s3","count","Integer","Duplicate copy of scanned package count",json!([17, 23, 31])),
field("s4","temperature","Float","Container air temperature in Celsius",json!([10, 12, 14])),
field("s5","zone","Text","Origin warehouse zone",json!(["Z1", "Z1", "Z1"])),
field("s6","timestamp","Timestamp","Undocumented container lifecycle instant",json!(null)),
],
target: vec![
field("t1","container_id","Text","Shipping container identity",json!(["CN-C4", "CN-A8", "CN-B9"])),
field("t2","package_count","Integer","Scanned packages in the container",json!([31, 17, 23])),
field("t3","temperature","Float","Container air temperature in Fahrenheit",json!([10, 12, 14])),
field("t4","zone","Text","Destination port zone",json!(["Z1", "Z1", "Z1"])),
field("t5","opened_at","Timestamp","Container opening instant",json!(null)),
field("t6","sealed_at","Timestamp","Container sealing instant",json!(null)),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identify the shipping container."),
label("s2",LabelKind::Match,&["t2"],"The first exported count records scanned packages."),
label("s3",LabelKind::Match,&["t2"],"The second count is an intentional duplicate fact; one-to-one assignment has a lower recall ceiling."),
label("s4",LabelKind::NoMatch,&[],"Celsius and Fahrenheit representations require conversion."),
label("s5",LabelKind::NoMatch,&[],"Origin warehouse and destination port zone labels have unrelated scopes."),
label("s6",LabelKind::Ambiguous,&["t5","t6"],"Both lifecycle timestamps fit the unspecified source instant."),
],},
Family {
id: "fulfillment_06".into(),
domain: "fulfillment".into(),
title: "Inter-depot transfer ledger".into(),
tags: vec!["renamed".into(),"event_dates".into(),"units".into(),"missing_samples".into()],
rationale: "A logistics service migrates inter-depot transfers. Route distance and arrival dates in a planning table are unsuitable replacements for differently represented source facts. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","transfer_ref","Text","Inter-depot transfer identity",json!(["TF-801", "TF-814", "TF-826"])),
field("s2","distance_km","Float","Route distance in kilometres",json!([80, 125, 47])),
field("s3","departure_date","Date","Actual departure date",json!(["2026-07-01", "2026-07-02", "2026-07-03"])),
field("s4","from","Text","Origin depot code",json!(["DP-N1", "DP-N2", "DP-N3"])),
field("s5","to","Text","Destination depot code",json!(["DP-S1", "DP-S2", "DP-S3"])),
field("s6","driver_notes","Text","Transfer driver note",json!(null)),
],
target: vec![
field("t1","transfer_id","Text","Inter-depot transfer identity",json!(["TF-826", "TF-801", "TF-814"])),
field("t2","distance_m","Float","Route distance in metres",json!([80, 125, 47])),
field("t3","arrival_date","Date","Actual arrival date",json!(["2026-07-01", "2026-07-02", "2026-07-03"])),
field("t4","origin_depot","Text","Origin depot code",json!(["DP-N3", "DP-N1", "DP-N2"])),
field("t5","destination_depot","Text","Destination depot code",json!(["DP-S3", "DP-S1", "DP-S2"])),
field("t6","operator_comment","Text","Transfer driver note",json!([])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identify the same transfer."),
label("s2",LabelKind::NoMatch,&[],"Kilometres and metres cannot be substituted without scaling."),
label("s3",LabelKind::NoMatch,&[],"Departure and arrival dates are different events even when same-day transfers make samples agree."),
label("s4",LabelKind::Match,&["t4"],"Both refer to the originating depot."),
label("s5",LabelKind::Match,&["t5"],"Both refer to the receiving depot."),
label("s6",LabelKind::Match,&["t6"],"The comments retain the driver-note meaning with no sample evidence."),
],},
Family {
id: "subscriptions_01".into(),
domain: "subscription_billing".into(),
title: "Recurring subscription invoices".into(),
tags: vec!["amount_scope".into(),"renamed".into(),"constant_currency".into()],
rationale: "A subscription platform imports recurring invoice summaries. Gross charges, net charges, pauses and cancellations have distinct business meanings. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","bill_ref","Text","Recurring invoice identity",json!(["BI-410", "BI-423", "BI-437"])),
field("s2","GrossMonthlyAmt","Decimal","Monthly charge including tax",json!([55, 110, 165])),
field("s3","billing_month","Date","First day of billed calendar month",json!(["2026-01-01", "2026-02-01", "2026-03-01"])),
field("s4","cancelled_on","Date","Effective subscription cancellation date",json!(["2026-04-01", "2026-04-05", "2026-04-09"])),
field("s5","Seats","Integer","Licensed seat count",json!([3, 7, 11])),
field("s6","currency","Text","Invoice denomination currency",json!(["EUR", "EUR", "EUR"])),
],
target: vec![
field("t1","invoice_id","Text","Recurring invoice identity",json!(["BI-437", "BI-410", "BI-423"])),
field("t2","monthly_net_charge","Decimal","Monthly charge excluding tax",json!([55, 110, 165])),
field("t3","period_month","Date","First day of billed calendar month",json!(["2026-03-01", "2026-01-01", "2026-02-01"])),
field("t4","pause_end","Date","Date a temporary service pause ends",json!(["2026-04-01", "2026-04-05", "2026-04-09"])),
field("t5","seat_count","Integer","Licensed seat count",json!([11, 3, 7])),
field("t6","currency_code","Text","Invoice denomination currency",json!(["EUR", "EUR", "EUR"])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identifiers name the recurring invoice."),
label("s2",LabelKind::NoMatch,&[],"Gross monthly charges cannot replace net charges without a tax transformation."),
label("s3",LabelKind::Match,&["t3"],"Both dates encode the beginning of the billed month."),
label("s4",LabelKind::NoMatch,&[],"Ending a pause is not cancelling service."),
label("s5",LabelKind::Match,&["t5"],"Both count licensed seats."),
label("s6",LabelKind::Match,&["t6"],"Both currency fields denominate the invoice; repetition is inherent to this sample."),
],},
Family {
id: "subscriptions_02".into(),
domain: "subscription_billing".into(),
title: "Metered service usage periods".into(),
tags: vec!["disjoint_samples".into(),"ambiguous_plan".into(),"amount_scope".into()],
rationale: "Usage records from two time slices share a meter schema. A generic plan column has no indication of whether it stores public or contracted plan labels. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","usage_ref","Text","Metered usage record identity",json!(["UG-11", "UG-12", "UG-13"])),
field("s2","period_start","Timestamp","Usage aggregation window start",json!(["2026-02-01T00:00:00Z", "2026-02-02T00:00:00Z", "2026-02-03T00:00:00Z"])),
field("s3","requests","Integer","Count of billed API requests",json!([140, 260, 380])),
field("s4","amount","Decimal","Charge for this usage window",json!([14, 26, 38])),
field("s5","currency","Text","Usage charge denomination",json!(["USD", "USD", "USD"])),
field("s6","plan","Text","Undocumented plan label",json!([])),
],
target: vec![
field("t1","usage_id","Text","Metered usage record identity",json!(["UG-21", "UG-22", "UG-23"])),
field("t2","window_begins_at","Timestamp","Usage aggregation window start",json!(["2026-02-04T00:00:00Z", "2026-02-05T00:00:00Z", "2026-02-06T00:00:00Z"])),
field("t3","request_count","Integer","Count of billed API requests",json!([410, 520, 630])),
field("t4","amount","Decimal","Entire account balance after billing",json!([14, 26, 38])),
field("t5","currency_code","Text","Usage charge denomination",json!(["USD", "USD", "USD"])),
field("t6","public_plan","Text","Public catalog plan label",json!([])),
field("t7","contract_plan","Text","Negotiated contract plan label",json!([])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Usage record identities use one namespace across separate time slices."),
label("s2",LabelKind::Match,&["t2"],"Both identify aggregation-window starts."),
label("s3",LabelKind::Match,&["t3"],"Both count requests in the same billing unit; different windows explain disjoint values."),
label("s4",LabelKind::NoMatch,&[],"A window charge is not the total account balance."),
label("s5",LabelKind::Match,&["t5"],"Both denominate usage charges."),
label("s6",LabelKind::Ambiguous,&["t6","t7"],"The source plan label lacks the distinction between public catalog and contracted plan."),
],},
Family {
id: "subscriptions_03".into(),
domain: "subscription_billing".into(),
title: "Refund processing import".into(),
tags: vec!["identifier_scope".into(),"misleading_status".into(),"unmatched_description".into()],
rationale: "A service imports processed subscription refunds. Customer and order identifiers may reuse the same short numbers, and a refund processing state differs from an eligibility decision. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","refund_ref","Text","Subscription refund identity",json!(["RF-301", "RF-312", "RF-323"])),
field("s2","id","Text","Refunded customer identity",json!(["41", "42", "43"])),
field("s3","value","Decimal","Money refunded in account currency",json!([15, 27, 39])),
field("s4","reason","Text","Human-readable refund explanation",json!(["duplicate", "service issue", "cancellation"])),
field("s5","status","Text","Refund processing state",json!(["done", "held", "done"])),
field("s6","processed_on","Date","Refund completion date",json!(["2026-03-04", "2026-03-08", "2026-03-12"])),
],
target: vec![
field("t1","refund_id","Text","Subscription refund identity",json!(["RF-323", "RF-301", "RF-312"])),
field("t2","id","Text","Purchase order identity",json!(["41", "42", "43"])),
field("t3","refund_amount","Decimal","Money refunded in account currency",json!([39, 15, 27])),
field("t4","reason_code","Integer","Internal refund reason enumeration",json!([1, 2, 3])),
field("t5","status","Text","Refund eligibility decision state",json!(["done", "held", "done"])),
field("t6","completed_date","Date","Refund completion date",json!(["2026-03-12", "2026-03-04", "2026-03-08"])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identify subscription refunds."),
label("s2",LabelKind::NoMatch,&[],"Customer and purchase order identities are different domains."),
label("s3",LabelKind::Match,&["t3"],"Both record the refunded amount without currency or tax conversion."),
label("s4",LabelKind::NoMatch,&[],"Replacing explanations with numeric codes requires a lookup transformation."),
label("s5",LabelKind::NoMatch,&[],"Processing state and eligibility decision are different facts."),
label("s6",LabelKind::Match,&["t6"],"Both dates record refund completion."),
],},
Family {
id: "subscriptions_04".into(),
domain: "subscription_billing".into(),
title: "Annual service commitments".into(),
tags: vec!["ambiguous_end".into(),"minimum_commitment".into(),"renamed".into()],
rationale: "An annual contract extract contains one undocumented end date. Renewal and termination are separate possibilities; minimum annual commitment is not a monthly service fee. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","agreement","Text","Subscription contract identity",json!(["AG-A5", "AG-B6", "AG-C7"])),
field("s2","start","Date","Contract commencement date",json!(["2026-01-10", "2026-02-10", "2026-03-10"])),
field("s3","end","Date","Undocumented contract end-like date",json!(null)),
field("s4","amount","Decimal","Annual minimum spend commitment",json!([1200, 2400, 3600])),
field("s5","licenses","Integer","Committed licensed seats",json!([10, 20, 30])),
field("s6","subscriber","Text","Contract subscriber account identity",json!(["AC-311", "AC-322", "AC-333"])),
],
target: vec![
field("t1","contract_id","Text","Subscription contract identity",json!(["AG-C7", "AG-A5", "AG-B6"])),
field("t2","effective_date","Date","Contract commencement date",json!(["2026-03-10", "2026-01-10", "2026-02-10"])),
field("t3","termination_date","Date","Date service terminates",json!(null)),
field("t4","renewal_date","Date","Date another contract term begins",json!(null)),
field("t5","amount","Decimal","Monthly service fee",json!([1200, 2400, 3600])),
field("t6","seat_commitment","Integer","Committed licensed seats",json!([30, 10, 20])),
field("t7","account_id","Text","Contract subscriber account identity",json!(["AC-333", "AC-311", "AC-322"])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identify the service contract."),
label("s2",LabelKind::Match,&["t2"],"Both record contract commencement."),
label("s3",LabelKind::Ambiguous,&["t3","t4"],"The source does not distinguish termination from the next renewal boundary."),
label("s4",LabelKind::NoMatch,&[],"Annual minimum spend and monthly fee have different period and commitment semantics."),
label("s5",LabelKind::Match,&["t6"],"Both count committed seats."),
label("s6",LabelKind::Match,&["t7"],"Both identify the subscriber account."),
],},
Family {
id: "subscriptions_05".into(),
domain: "subscription_billing".into(),
title: "Pricing revision catalog".into(),
tags: vec!["duplicate_names".into(),"disjoint_ids".into(),"constant_currency".into(),"boolean_scope".into()],
rationale: "A catalog publishes revisions with separate wholesale and retail prices under duplicate headings. Plan identifiers are sampled from different plans; a retirement flag is not a service-enabled flag. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","plan_key","Text","Public subscription plan identity",json!(["PL-A1", "PL-A2", "PL-A3"])),
field("s2","effective_from","Date","Pricing revision effective date",json!(["2026-04-01", "2026-05-01", "2026-06-01"])),
field("s3","price","Decimal","Wholesale plan price per month",json!([8, 16, 24])),
field("s4","price","Decimal","Retail plan price per month",json!([12, 22, 32])),
field("s5","currency","Text","Price denomination currency",json!(["GBP", "GBP", "GBP"])),
field("s6","enabled","Boolean","Service currently enabled",json!([true, false, true])),
],
target: vec![
field("t1","plan_id","Text","Public subscription plan identity",json!(["PL-B4", "PL-B5", "PL-B6"])),
field("t2","valid_from","Date","Pricing revision effective date",json!(["2026-06-01", "2026-04-01", "2026-05-01"])),
field("t3","price","Decimal","Retail plan price per month",json!([32, 12, 22])),
field("t4","price","Decimal","Wholesale plan price per month",json!([24, 8, 16])),
field("t5","currency_code","Text","Price denomination currency",json!(["GBP", "GBP", "GBP"])),
field("t6","enabled","Boolean","Legacy plan retired from sale",json!([true, false, true])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identifiers belong to the public-plan namespace despite disjoint sample rows."),
label("s2",LabelKind::Match,&["t2"],"Both dates govern the pricing revision."),
label("s3",LabelKind::Match,&["t4"],"This source price records wholesale charges."),
label("s4",LabelKind::Match,&["t3"],"This source price records retail charges."),
label("s5",LabelKind::Match,&["t5"],"Both currency fields denominate the plan prices."),
label("s6",LabelKind::NoMatch,&[],"Enabled service and retirement from new sales are distinct Boolean facts."),
],},
Family {
id: "subscriptions_06".into(),
domain: "subscription_billing".into(),
title: "Prepaid credit ledger".into(),
tags: vec!["amount_scope".into(),"ambiguous_account".into(),"event_dates".into()],
rationale: "A prepaid ledger stores entry deltas separately from balances. A generic party key lacks the distinction between service subscriber and bill-paying organization. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","entry","Text","Credit ledger entry identity",json!(["LE-610", "LE-621", "LE-632"])),
field("s2","amount","Decimal","Credit balance after this entry",json!([40, 60, 80])),
field("s3","delta","Decimal","Signed change in prepaid credit",json!([-5, 10, -15])),
field("s4","posted_at","Timestamp","Ledger posting instant",json!(["2026-06-01T01:01:00Z", "2026-06-02T02:02:00Z", "2026-06-03T03:03:00Z"])),
field("s5","settled_on","Date","External payment settlement date",json!(["2026-06-01", "2026-06-02", "2026-06-03"])),
field("s6","party_id","Text","Undocumented subscriber or payer key",json!([])),
],
target: vec![
field("t1","ledger_id","Text","Credit ledger entry identity",json!(["LE-632", "LE-610", "LE-621"])),
field("t2","amount","Decimal","Net charge for a service purchase",json!([40, 60, 80])),
field("t3","credit_change","Decimal","Signed change in prepaid credit",json!([-15, -5, 10])),
field("t4","recorded_at","Timestamp","Ledger posting instant",json!(["2026-06-03T03:03:00Z", "2026-06-01T01:01:00Z", "2026-06-02T02:02:00Z"])),
field("t5","settled_on","Date","Date subscription invoice was issued",json!(["2026-06-01", "2026-06-02", "2026-06-03"])),
field("t6","subscriber_id","Text","Service subscriber identity",json!([])),
field("t7","payer_id","Text","Bill-paying organization identity",json!([])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identify the same ledger entry."),
label("s2",LabelKind::NoMatch,&[],"Credit balance and a purchase charge are different amounts."),
label("s3",LabelKind::Match,&["t3"],"Both record the signed prepaid credit delta in the same currency."),
label("s4",LabelKind::Match,&["t4"],"Both timestamps record ledger posting."),
label("s5",LabelKind::NoMatch,&[],"External settlement and invoice issuance are different events."),
label("s6",LabelKind::Ambiguous,&["t6","t7"],"The source key does not document whether the party subscribes to or pays for service."),
],},
Family {
id: "laboratory_01".into(),
domain: "research_labs".into(),
title: "Environmental water-sample register".into(),
tags: vec!["units".into(),"disjoint_samples".into(),"workflow_scope".into()],
rationale: "A nonclinical water-analysis laboratory transfers specimen metadata and measurements. Conductivity representations differ, and instrument state must not replace sample workflow state. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","bottle_ref","Text","Water specimen identity",json!(["WA-011", "WA-024", "WA-037"])),
field("s2","collected_on","Date","Water specimen collection date",json!(["2026-01-02", "2026-01-04", "2026-01-06"])),
field("s3","pH","Float","Measured specimen pH",json!([6.8, 7.1, 7.4])),
field("s4","conductivity","Float","Conductivity in microsiemens per centimetre",json!([110, 220, 330])),
field("s5","temperature_c","Float","Specimen temperature in Celsius",json!([12.1, 13.2, 14.3])),
field("s6","status","Text","Specimen laboratory workflow state",json!(["ready", "held", "ready"])),
],
target: vec![
field("t1","specimen_id","Text","Water specimen identity",json!(["WA-037", "WA-011", "WA-024"])),
field("t2","collection_date","Date","Water specimen collection date",json!(["2026-01-06", "2026-01-02", "2026-01-04"])),
field("t3","acidity_ph","Float","Measured specimen pH",json!([7.4, 6.8, 7.1])),
field("t4","conductivity","Float","Conductivity in siemens per centimetre",json!([110, 220, 330])),
field("t5","sample_temperature_c","Float","Specimen temperature in Celsius",json!([15.4, 16.5, 17.6])),
field("t6","status","Text","Instrument operating state",json!(["ready", "held", "ready"])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both fields identify the same water specimen."),
label("s2",LabelKind::Match,&["t2"],"Both describe specimen collection date."),
label("s3",LabelKind::Match,&["t3"],"Both measure pH without a unit conversion."),
label("s4",LabelKind::NoMatch,&[],"Microsiemens and siemens require scaling."),
label("s5",LabelKind::Match,&["t5"],"Both represent specimen temperature in Celsius; sample rows are disjoint."),
label("s6",LabelKind::NoMatch,&[],"Specimen workflow and instrument operation are independent states."),
],},
Family {
id: "laboratory_02".into(),
domain: "research_labs".into(),
title: "Materials incubator runs".into(),
tags: vec!["ambiguous_measurement".into(),"duration_units".into(),"missing_samples".into()],
rationale: "A materials laboratory imports incubator run logs. The source temperature heading does not specify room air or chamber interior, while duration units are documented and different. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","run_ref","Text","Materials incubation run identity",json!(["IR-151", "IR-162", "IR-173"])),
field("s2","duration","Float","Incubation duration in minutes",json!([15, 30, 45])),
field("s3","operator","Text","Laboratory operator code",json!(["OP-A1", "OP-B2", "OP-C3"])),
field("s4","completed","Boolean","Incubation run completed",json!([true, false, true])),
field("s5","temperature","Float","Undocumented room or chamber temperature",json!(null)),
field("s6","notes","Text","Operator run comment",json!([])),
],
target: vec![
field("t1","run_id","Text","Materials incubation run identity",json!(["IR-173", "IR-151", "IR-162"])),
field("t2","duration","Float","Incubation duration in seconds",json!([15, 30, 45])),
field("t3","operator_id","Text","Laboratory operator code",json!(["OP-C3", "OP-A1", "OP-B2"])),
field("t4","is_complete","Boolean","Incubation run completed",json!([true, true, false])),
field("t5","room_temperature","Float","Room air temperature",json!(null)),
field("t6","chamber_temperature","Float","Incubator interior temperature",json!(null)),
field("t7","run_comment","Text","Operator run comment",json!(null)),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identify the incubation run."),
label("s2",LabelKind::NoMatch,&[],"Duration needs conversion between minutes and seconds."),
label("s3",LabelKind::Match,&["t3"],"Both identify the operator in the laboratory code system."),
label("s4",LabelKind::Match,&["t4"],"Both indicate completion of the run."),
label("s5",LabelKind::Ambiguous,&["t5","t6"],"No supplied evidence establishes which environment the source temperature measures."),
label("s6",LabelKind::Match,&["t7"],"Both preserve the operator comment, despite missing observations."),
],},
Family {
id: "laboratory_03".into(),
domain: "research_labs".into(),
title: "Calibration standard readings".into(),
tags: vec!["duplicate_names".into(),"event_dates".into(),"units".into()],
rationale: "An instrument calibration register exports nominal standard concentration and observed concentration under the same heading. Certificate expiry is not calibration date. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","standard","Text","Calibration standard identity",json!(["ST-D4", "ST-E5", "ST-F6"])),
field("s2","value","Float","Nominal standard concentration in ppm",json!([10, 20, 30])),
field("s3","value","Float","Observed instrument concentration in ppm",json!([9.8, 20.2, 29.7])),
field("s4","date","Date","Calibration execution date",json!(["2026-03-02", "2026-03-05", "2026-03-08"])),
field("s5","tolerance","Float","Allowed absolute deviation in ppm",json!([0.3, 0.4, 0.5])),
field("s6","unit","Text","Concentration unit label",json!(["ppm", "ppm", "ppm"])),
],
target: vec![
field("t1","standard_id","Text","Calibration standard identity",json!(["ST-F6", "ST-D4", "ST-E5"])),
field("t2","value","Float","Observed instrument concentration in ppm",json!([29.7, 9.8, 20.2])),
field("t3","value","Float","Nominal standard concentration in ppm",json!([30, 10, 20])),
field("t4","date","Date","Calibration certificate expiry date",json!(["2026-03-02", "2026-03-05", "2026-03-08"])),
field("t5","allowed_error","Float","Allowed absolute deviation in ppm",json!([0.5, 0.3, 0.4])),
field("t6","unit","Text","Target report concentration unit label",json!(["percent", "percent", "percent"])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identify the calibration standard."),
label("s2",LabelKind::Match,&["t3"],"The nominal concentration is distinct from an instrument reading."),
label("s3",LabelKind::Match,&["t2"],"This source value is the observed instrument reading."),
label("s4",LabelKind::NoMatch,&[],"Execution and certificate expiry dates describe different events."),
label("s5",LabelKind::Match,&["t5"],"Both values express the same allowable deviation in ppm."),
label("s6",LabelKind::NoMatch,&[],"These labels specify different concentration representations, rather than equivalent unit metadata."),
],},
Family {
id: "laboratory_04".into(),
domain: "research_labs".into(),
title: "Field weather-trial observations".into(),
tags: vec!["ambiguous_temperature".into(),"units".into(),"boolean_scope".into()],
rationale: "An outdoor materials trial records weather observations. A temperature column has no distinction between air and dew-point readings; a maintenance flag is unrelated to observation validity. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","station","Text","Trial weather-station identity",json!(["WS-J1", "WS-K2", "WS-L3"])),
field("s2","temperature","Float","Undocumented air or dew-point temperature",json!([])),
field("s3","wind_speed","Float","Wind speed in miles per hour",json!([4, 8, 12])),
field("s4","rain_mm","Float","Accumulated rainfall in millimetres",json!([0.5, 1.5, 2.5])),
field("s5","valid","Boolean","Observation passed validation",json!([true, false, true])),
field("s6","day","Date","Weather observation date",json!(["2026-04-03", "2026-04-06", "2026-04-09"])),
],
target: vec![
field("t1","station_id","Text","Trial weather-station identity",json!(["WS-L3", "WS-J1", "WS-K2"])),
field("t2","air_temperature","Float","Ambient air temperature",json!([])),
field("t3","dewpoint_temperature","Float","Dew-point temperature",json!([])),
field("t4","wind_speed","Float","Wind speed in metres per second",json!([4, 8, 12])),
field("t5","rainfall_mm","Float","Accumulated rainfall in millimetres",json!([2.5, 0.5, 1.5])),
field("t6","valid","Boolean","Station maintenance currently enabled",json!([true, false, true])),
field("t7","observed_date","Date","Weather observation date",json!(["2026-04-09", "2026-04-03", "2026-04-06"])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identify a station in the trial."),
label("s2",LabelKind::Ambiguous,&["t2","t3"],"Both temperature facts remain plausible for the undocumented source."),
label("s3",LabelKind::NoMatch,&[],"Wind-speed units require conversion."),
label("s4",LabelKind::Match,&["t5"],"Both fields measure rainfall in millimetres."),
label("s5",LabelKind::NoMatch,&[],"Observation validity is not a station-maintenance setting."),
label("s6",LabelKind::Match,&["t7"],"Both fields give the observation date."),
],},
Family {
id: "laboratory_05".into(),
domain: "research_labs".into(),
title: "Materials microscopy image catalog".into(),
tags: vec!["dimensions".into(),"event_times".into(),"disjoint_samples".into()],
rationale: "A materials microscope catalog moves image metadata to an archive. Pixel dimensions must not be confused with physical dimensions, and session startup differs from frame acquisition. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","image_ref","Text","Microscopy image identity",json!(["IM-611", "IM-622", "IM-633"])),
field("s2","specimen","Text","Material specimen code",json!(["SP-A4", "SP-B5", "SP-C6"])),
field("s3","width","Integer","Image width in pixels",json!([512, 1024, 2048])),
field("s4","height_px","Integer","Image height in pixels",json!([256, 512, 1024])),
field("s5","channel","Text","Optical acquisition channel",json!(["red", "green", "blue"])),
field("s6","session_start","Timestamp","Instrument session startup instant",json!(["2026-05-01T01:00:00Z", "2026-05-02T02:00:00Z", "2026-05-03T03:00:00Z"])),
],
target: vec![
field("t1","image_id","Text","Microscopy image identity",json!(["IM-633", "IM-611", "IM-622"])),
field("t2","sample_label","Text","Material specimen code",json!(["SP-D7", "SP-E8", "SP-F9"])),
field("t3","width","Float","Physical imaged width in millimetres",json!([512, 1024, 2048])),
field("t4","pixel_height","Integer","Image height in pixels",json!([1024, 256, 512])),
field("t5","optical_channel","Text","Optical acquisition channel",json!(["blue", "red", "green"])),
field("t6","session_start","Timestamp","Frame acquisition instant",json!(["2026-05-01T01:00:00Z", "2026-05-02T02:00:00Z", "2026-05-03T03:00:00Z"])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identify the microscope image."),
label("s2",LabelKind::Match,&["t2"],"Both identify material specimens, sampled from different image rows."),
label("s3",LabelKind::NoMatch,&[],"Pixel count cannot substitute for physical width without calibration."),
label("s4",LabelKind::Match,&["t4"],"Both fields count vertical pixels."),
label("s5",LabelKind::Match,&["t5"],"Both identify the optical acquisition channel."),
label("s6",LabelKind::NoMatch,&[],"Instrument session startup and frame acquisition are separate events."),
],},
Family {
id: "laboratory_06".into(),
domain: "research_labs".into(),
title: "Soil plot measurement survey".into(),
tags: vec!["duplicate_names".into(),"units".into(),"ambiguous_quality".into()],
rationale: "A soil-research survey exports moisture and salinity under duplicate reading headings. An undocumented quality score could refer to measurement quality or a soil condition index. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","plot","Text","Research plot identity",json!(["PT-71", "PT-82", "PT-93"])),
field("s2","reading","Float","Volumetric soil moisture percentage",json!([12, 18, 24])),
field("s3","reading","Float","Soil salinity in a fixed instrument scale",json!([1.2, 1.8, 2.4])),
field("s4","survey_day","Date","Field survey date",json!(["2026-06-02", "2026-06-05", "2026-06-08"])),
field("s5","depth","Float","Sampling depth in centimetres",json!([10, 20, 30])),
field("s6","quality","Float","Undocumented measurement-quality or soil-condition score",json!(null)),
],
target: vec![
field("t1","plot_id","Text","Research plot identity",json!(["PT-93", "PT-71", "PT-82"])),
field("t2","reading","Float","Soil salinity in a fixed instrument scale",json!([2.4, 1.2, 1.8])),
field("t3","reading","Float","Volumetric soil moisture percentage",json!([24, 12, 18])),
field("t4","survey_date","Date","Field survey date",json!(["2026-06-08", "2026-06-02", "2026-06-05"])),
field("t5","depth","Float","Sampling depth in millimetres",json!([10, 20, 30])),
field("t6","measurement_quality","Float","Measurement quality score",json!(null)),
field("t7","soil_quality","Float","Soil condition index",json!(null)),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identify the surveyed plot."),
label("s2",LabelKind::Match,&["t3"],"This reading is soil moisture, not salinity."),
label("s3",LabelKind::Match,&["t2"],"This reading is soil salinity in the same instrument scale."),
label("s4",LabelKind::Match,&["t4"],"Both identify the survey day."),
label("s5",LabelKind::NoMatch,&[],"Sampling depth requires centimetre-to-millimetre conversion."),
label("s6",LabelKind::Ambiguous,&["t6","t7"],"The source quality score lacks the distinction between measurement and environmental quality."),
],},
Family {
id: "civic_01".into(),
domain: "civic_records".into(),
title: "Municipal permit intake".into(),
tags: vec!["payment_scope".into(),"workflow_scope".into(),"renamed".into()],
rationale: "A fictitious municipality moves permit intake records to a review system. Charged fees differ from paid fees, and intake status differs from inspection status. All references and addresses are invented. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","permit_ref","Text","Permit application identity",json!(["PE-A1", "PE-B2", "PE-C3"])),
field("s2","filed_on","Date","Permit submission date",json!(["2026-01-03", "2026-01-06", "2026-01-09"])),
field("s3","fee","Decimal","Permit fee charged to applicant",json!([20, 40, 60])),
field("s4","applicant_ref","Text","Permit applicant reference",json!(["AP-701", "AP-712", "AP-723"])),
field("s5","location","Text","Permitted project site label",json!(["Site Alder", "Site Birch", "Site Cedar"])),
field("s6","status","Text","Permit intake workflow state",json!(["open", "held", "open"])),
],
target: vec![
field("t1","application_id","Text","Permit application identity",json!(["PE-C3", "PE-A1", "PE-B2"])),
field("t2","submitted_date","Date","Permit submission date",json!(["2026-01-09", "2026-01-03", "2026-01-06"])),
field("t3","fee","Decimal","Permit fee actually paid",json!([20, 40, 60])),
field("t4","applicant_id","Text","Permit applicant reference",json!(["AP-723", "AP-701", "AP-712"])),
field("t5","site_address","Text","Permitted project site label",json!(["Site Cedar", "Site Alder", "Site Birch"])),
field("t6","status","Text","Project inspection workflow state",json!(["open", "held", "open"])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identify the permit application."),
label("s2",LabelKind::Match,&["t2"],"Both record when the application was submitted."),
label("s3",LabelKind::NoMatch,&[],"Charged and actually paid fees can differ and are separate facts."),
label("s4",LabelKind::Match,&["t4"],"Both refer to the permit applicant using the same reference system."),
label("s5",LabelKind::Match,&["t5"],"Both labels identify the project site."),
label("s6",LabelKind::NoMatch,&[],"Intake workflow and inspection workflow must not be conflated."),
],},
Family {
id: "civic_02".into(),
domain: "civic_records".into(),
title: "Community-center room reservations".into(),
tags: vec!["ambiguous_start".into(),"constant_scope".into(),"deposit_scope".into()],
rationale: "A fictitious community center imports room bookings. A start time could describe room setup or the public event, while a room code from another building uses an unrelated namespace. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","reservation","Text","Community reservation identity",json!(["BK-81", "BK-92", "BK-03"])),
field("s2","booked_on","Date","Date the reservation was placed",json!(["2026-02-02", "2026-02-05", "2026-02-08"])),
field("s3","start","Timestamp","Undocumented setup or public-event start",json!([])),
field("s4","attendees","Integer","Expected event attendance",json!([15, 25, 35])),
field("s5","deposit","Decimal","Refundable booking deposit",json!([30, 50, 70])),
field("s6","room","Text","Room code within the north center",json!(["R1", "R1", "R1"])),
],
target: vec![
field("t1","booking_id","Text","Community reservation identity",json!(["BK-03", "BK-81", "BK-92"])),
field("t2","reservation_date","Date","Date the reservation was placed",json!(["2026-02-08", "2026-02-02", "2026-02-05"])),
field("t3","setup_start","Timestamp","Beginning of room setup",json!([])),
field("t4","event_start","Timestamp","Beginning of public event",json!([])),
field("t5","guest_count","Integer","Expected event attendance",json!([35, 15, 25])),
field("t6","deposit","Decimal","Nonrefundable room-hire fee",json!([30, 50, 70])),
field("t7","room","Text","Room code within the south center",json!(["R1", "R1", "R1"])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identify the same reservation."),
label("s2",LabelKind::Match,&["t2"],"Both record booking placement rather than the event day."),
label("s3",LabelKind::Ambiguous,&["t3","t4"],"The source start time does not specify setup versus public event."),
label("s4",LabelKind::Match,&["t5"],"Both count expected attendees."),
label("s5",LabelKind::NoMatch,&[],"Refundable deposits and room-hire fees have different obligations."),
label("s6",LabelKind::NoMatch,&[],"Equal room codes from distinct center namespaces identify different rooms."),
],},
Family {
id: "civic_03".into(),
domain: "civic_records".into(),
title: "Public-library copy catalog".into(),
tags: vec!["disjoint_samples".into(),"missing_samples".into(),"misleading_status".into()],
rationale: "A fictional library migrates its copy catalog. Publication metadata survives renaming and disjoint examples, while condition and circulation state are distinct. No real patron data is present. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","barcode","Text","Physical library copy identity",json!(["LC-411", "LC-422", "LC-433"])),
field("s2","title","Text","Published work title",json!(["The Copper Kite", "Map of Pines", "Cloud Atlas Manual"])),
field("s3","published_year","Integer","Year the work was published",json!(null)),
field("s4","status","Text","Copy circulation state",json!(["available", "held", "available"])),
field("s5","fee","Decimal","Accrued overdue fine",json!([2, 4, 6])),
field("s6","author","Text","Work creator display label",json!(["Writer Amber", "Writer Bronze", "Writer Cobalt"])),
],
target: vec![
field("t1","copy_id","Text","Physical library copy identity",json!(["LC-433", "LC-411", "LC-422"])),
field("t2","display_title","Text","Published work title",json!(["River Workshop", "Stone Pattern Book", "Meadow Guide"])),
field("t3","publication_year","Integer","Year the work was published",json!([])),
field("t4","status","Text","Physical copy condition category",json!(["available", "held", "available"])),
field("t5","fee","Decimal","Replacement purchase cost",json!([2, 4, 6])),
field("t6","creator_name","Text","Work creator display label",json!(["Writer Cobalt", "Writer Amber", "Writer Bronze"])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identify the physical copy rather than the abstract work."),
label("s2",LabelKind::Match,&["t2"],"Both store published titles, with observations taken from different works."),
label("s3",LabelKind::Match,&["t3"],"Both store publication year even without observations."),
label("s4",LabelKind::NoMatch,&[],"Circulation state and physical condition describe different aspects of a copy."),
label("s5",LabelKind::NoMatch,&[],"Overdue fines and replacement purchase cost are different amounts."),
label("s6",LabelKind::Match,&["t6"],"Both fields label the work creator."),
],},
Family {
id: "civic_04".into(),
domain: "civic_records".into(),
title: "Road-maintenance work orders".into(),
tags: vec!["estimated_actual".into(),"event_dates".into(),"missing_samples".into()],
rationale: "A fictitious roads office transfers maintenance work orders. Estimated costs and observed expenditure differ, as do work commencement and inspection dates. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","job_ref","Text","Road maintenance task identity",json!(["RJ-521", "RJ-532", "RJ-543"])),
field("s2","segment","Text","Road segment code",json!(["RD-A2", "RD-B3", "RD-C4"])),
field("s3","distance_m","Float","Length of segment worked in metres",json!([80, 140, 200])),
field("s4","cost","Decimal","Estimated maintenance cost",json!([1000, 2000, 3000])),
field("s5","start_date","Date","Actual work commencement date",json!(["2026-04-02", "2026-04-05", "2026-04-08"])),
field("s6","remarks","Text","Crew work-order note",json!(null)),
],
target: vec![
field("t1","task_id","Text","Road maintenance task identity",json!(["RJ-543", "RJ-521", "RJ-532"])),
field("t2","road_segment_id","Text","Road segment code",json!(["RD-C4", "RD-A2", "RD-B3"])),
field("t3","worked_length_m","Float","Length of segment worked in metres",json!([200, 80, 140])),
field("t4","cost","Decimal","Actual maintenance expenditure",json!([1000, 2000, 3000])),
field("t5","start_date","Date","Site inspection date",json!(["2026-04-02", "2026-04-05", "2026-04-08"])),
field("t6","work_notes","Text","Crew work-order note",json!([])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identify the maintenance task."),
label("s2",LabelKind::Match,&["t2"],"Both identify the road segment."),
label("s3",LabelKind::Match,&["t3"],"Both lengths use metres and the same worked portion."),
label("s4",LabelKind::NoMatch,&[],"Estimated and actual cost are separate accounting facts."),
label("s5",LabelKind::NoMatch,&[],"Work commencement and inspection dates are distinct events."),
label("s6",LabelKind::Match,&["t6"],"Both preserve the crew note without sample support."),
],},
Family {
id: "civic_05".into(),
domain: "civic_records".into(),
title: "Business-license renewals".into(),
tags: vec!["ambiguous_expiry".into(),"identifier_scope".into(),"renamed".into()],
rationale: "A fictitious licensing office imports renewal transactions. An expiry date has no documented subject, and licensing-authority identifiers may coincide with holder identifiers. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","license","Text","Business license identity",json!(["BL-641", "BL-652", "BL-663"])),
field("s2","renewal","Integer","Renewal sequence number",json!([2, 4, 6])),
field("s3","paid_on","Date","Renewal payment date",json!(["2026-05-01", "2026-05-04", "2026-05-07"])),
field("s4","expiry","Date","Undocumented license or insurance expiry",json!(null)),
field("s5","id","Text","Issuing licensing authority identity",json!(["17", "18", "19"])),
field("s6","service_fee","Decimal","Renewal administration fee",json!([25, 35, 45])),
],
target: vec![
field("t1","license_id","Text","Business license identity",json!(["BL-663", "BL-641", "BL-652"])),
field("t2","sequence_no","Integer","Renewal sequence number",json!([6, 2, 4])),
field("t3","payment_date","Date","Renewal payment date",json!(["2026-05-07", "2026-05-01", "2026-05-04"])),
field("t4","license_expiry","Date","Business license expiry",json!(null)),
field("t5","insurance_expiry","Date","Holder liability insurance expiry",json!(null)),
field("t6","id","Text","License holder identity",json!(["17", "18", "19"])),
field("t7","administration_amount","Decimal","Renewal administration fee",json!([45, 25, 35])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identify the business license."),
label("s2",LabelKind::Match,&["t2"],"Both count renewal sequence positions."),
label("s3",LabelKind::Match,&["t3"],"Both record the renewal payment date."),
label("s4",LabelKind::Ambiguous,&["t4","t5"],"The source expiry date does not identify whether it applies to the license or its required insurance."),
label("s5",LabelKind::NoMatch,&[],"Issuing authorities and license holders are separate entity domains."),
label("s6",LabelKind::Match,&["t7"],"Both amounts are the renewal administration fee."),
],},
Family {
id: "civic_06".into(),
domain: "civic_records".into(),
title: "Municipal fleet asset inventory".into(),
tags: vec!["duplicate_source_facts".into(),"type_conflict".into(),"units".into()],
rationale: "A fictitious municipal fleet export repeats its registration field. The target contains one registration, passenger capacity instead of load capacity, and an availability Boolean instead of lifecycle status. Original synthetic scenario authored independently for the corrective cycle; all sample values and identifiers are invented. No matching outputs were used to assign truth.".into(),
source: vec![
field("s1","asset_ref","Text","Fleet vehicle asset identity",json!(["FV-711", "FV-722", "FV-733"])),
field("s2","registration","Text","Vehicle registration identifier",json!(["REG-A4", "REG-B5", "REG-C6"])),
field("s3","registration","Text","Duplicate copy of vehicle registration identifier",json!(["REG-A4", "REG-B5", "REG-C6"])),
field("s4","capacity","Float","Maximum cargo load in kilograms",json!([500, 1000, 1500])),
field("s5","status","Text","Asset lifecycle category",json!(["active", "repair", "retired"])),
field("s6","mileage","Float","Cumulative odometer distance in kilometres",json!([12000, 24000, 36000])),
],
target: vec![
field("t1","vehicle_id","Text","Fleet vehicle asset identity",json!(["FV-733", "FV-711", "FV-722"])),
field("t2","registration_id","Text","Vehicle registration identifier",json!(["REG-C6", "REG-A4", "REG-B5"])),
field("t3","capacity","Integer","Maximum passenger seat count",json!([5, 10, 15])),
field("t4","status","Boolean","Vehicle available for immediate use",json!([true, false, false])),
field("t5","odometer_km","Float","Cumulative odometer distance in kilometres",json!([36000, 12000, 24000])),
field("t6","paint_colour","Text","Vehicle exterior paint label",json!(["white", "blue", "green"])),
],
labels: vec![
label("s1",LabelKind::Match,&["t1"],"Both identify the fleet vehicle asset."),
label("s2",LabelKind::Match,&["t2"],"The first source registration is the same vehicle identifier."),
label("s3",LabelKind::Match,&["t2"],"The repeated source registration is another copy of the same fact; one-to-one assignment cannot retain both."),
label("s4",LabelKind::NoMatch,&[],"Cargo mass and passenger seat count are different quantities, not interchangeable numeric capacities."),
label("s5",LabelKind::NoMatch,&[],"Lifecycle categories cannot directly replace immediate availability booleans."),
label("s6",LabelKind::Match,&["t5"],"Both odometers measure cumulative kilometres."),
],},
];
    let mut ids = BTreeSet::new();
    for family in &families {
        crate::corpus::validate_family(family)?;
        if !ids.insert(family.id.as_str()) || !DOMAINS.contains(&family.domain.as_str()) {
            return Err("duplicate corrective family or unknown domain".into());
        }
    }
    if families.len() != 24 || HOLDOUT.iter().any(|id| !ids.contains(id)) {
        return Err("incomplete corrective family partition".into());
    }
    for domain in DOMAINS {
        let total = families.iter().filter(|f| f.domain == domain).count();
        let held = families
            .iter()
            .filter(|f| f.domain == domain && HOLDOUT.contains(&f.id.as_str()))
            .count();
        if total != 6 || held != 3 {
            return Err(
                "each corrective domain requires six families and three held-out families".into(),
            );
        }
    }
    families.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(families)
}

fn shuffle<T>(values: &mut [T], mut state: u64) {
    for upper in (1..values.len()).rev() {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        values.swap(upper, ((state >> 32) % ((upper + 1) as u64)) as usize);
    }
}

pub fn cases() -> Result<Vec<Case>, String> {
    let mut cases = Vec::new();
    for (family_index, family) in families()?.into_iter().enumerate() {
        for (variant_index, variant) in VARIANTS.iter().enumerate() {
            let seed = SEED + (family_index as u64) * 10 + variant_index as u64;
            let mut source = family.source.clone();
            let mut target = family.target.clone();
            if *variant == "reordered" {
                shuffle(&mut source, seed);
                shuffle(&mut target, seed ^ 0x9e37_79b9);
            } else if *variant == "null_heavy" {
                for field in source.iter_mut().chain(&mut target) {
                    if let Some(samples) = &mut field.samples {
                        samples.extend(std::iter::repeat_n(Value::Null, samples.len() * 3));
                    }
                }
            }
            cases.push(Case {
                id: format!("{}__{}", family.id, variant),
                family_id: family.id.clone(),
                domain: family.domain.clone(),
                tags: family.tags.clone(),
                variant: (*variant).into(),
                split: if HOLDOUT.contains(&family.id.as_str()) {
                    "holdout"
                } else {
                    "development"
                }
                .into(),
                seed,
                source,
                target,
                labels: family.labels.clone(),
            });
        }
    }
    Ok(cases)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_truth_and_partition_are_complete() {
        let families = families().unwrap();
        assert_eq!(families.len(), 24);
        assert_eq!(families.iter().map(|f| f.labels.len()).sum::<usize>(), 144);
        assert_eq!(HOLDOUT.into_iter().collect::<BTreeSet<_>>().len(), 12);
        for family in &families {
            assert!(family
                .labels
                .iter()
                .any(|label| label.kind == LabelKind::Match));
            assert!(family
                .labels
                .iter()
                .any(|label| label.kind == LabelKind::NoMatch));
        }
    }

    #[test]
    fn variants_are_reproducible_and_preserve_family_truth() {
        let cases = cases().unwrap();
        assert_eq!(cases.len(), 72);
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.split == "development")
                .count(),
            36
        );
        assert_eq!(
            cases.iter().filter(|case| case.split == "holdout").count(),
            36
        );
        assert_eq!(
            serde_json::to_string(&cases).unwrap(),
            serde_json::to_string(&super::cases().unwrap()).unwrap()
        );
        for group in cases.chunks_exact(3) {
            for variant in &group[1..] {
                assert_eq!(variant.family_id, group[0].family_id);
                assert_eq!(variant.split, group[0].split);
                assert_eq!(
                    serde_json::to_string(&variant.labels).unwrap(),
                    serde_json::to_string(&group[0].labels).unwrap()
                );
            }
            for fields in [&group[0].source, &group[0].target] {
                assert_eq!(
                    fields.len(),
                    fields
                        .iter()
                        .map(|field| &field.id)
                        .collect::<BTreeSet<_>>()
                        .len()
                );
            }
        }
    }

    #[test]
    fn null_variant_preserves_non_null_values_and_missingness() {
        for group in cases().unwrap().chunks_exact(3) {
            for (original, changed) in group[0]
                .source
                .iter()
                .chain(&group[0].target)
                .zip(group[2].source.iter().chain(&group[2].target))
            {
                match (&original.samples, &changed.samples) {
                    (None, None) => {}
                    (Some(original), Some(changed)) => {
                        assert_eq!(changed.len(), original.len() * 4);
                        assert_eq!(&changed[..original.len()], original);
                        assert!(changed[original.len()..].iter().all(Value::is_null));
                    }
                    _ => panic!("null-heavy transformation changed unavailable sample state"),
                }
            }
        }
    }
}
