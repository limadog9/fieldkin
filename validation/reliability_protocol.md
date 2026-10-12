# Blind suggestion reliability comparison

The three source teams annotated publisher documents independently of the
implementation author. They did not inspect matcher source, existing evaluation
fixtures, or predictions. Their selection policies, raw documents, field-level
evidence, extractor scripts, and freeze manifests are archived under
`eval_reliability/sources/`. Source-only annotation corrections are documented
before predictions; a correction is never selected using a matcher score.

The integration manifest `reliability_corpus.json` binds every evaluated case to
an exact member of an original frozen array and binds all source/evidence bytes
to SHA-256. The existing strict labeled-case parser validates every field and
answer, while the reliability adapter verifies fingerprints, complete membership,
and source hashes before prediction. Offline extraction is independently rerun in
temporary copies by `reliability/verify_extraction.py`, then compared byte-for-byte
with the frozen files. No generic schema interpretation or third-party parsing
dependency was added to the evaluator.

The intervention must be frozen before the implementation author sees these
inputs, labels, or predictions. Evaluate the unchanged before binary and the
frozen current implementation on the same case files with `Config::default()`.
No threshold sweep, sample editing, label editing, case removal, or additional
runtime tuning follows exposure to these predictions. The measured report records
the exact runtime files, input manifest hashes, and the first before/after results.

Use the established `evaluate_all` decision metrics unchanged: exact decision
accuracy; precision of unique automatic matches; recall of unique match labels;
automatic-match coverage; NoMatch/Ambiguous abstentions; wrong automatic matches;
automatic selections on expected NoMatch/Ambiguous; missed matches; and incorrect
ambiguities. An ambiguous answer is correct only if the returned target set is
exactly the labeled set. Correct abstentions do not inflate automatic precision.
Report each dataset and domain alongside pooled counts.

The opt-in `--reliability` report is separate from the old development and
independent measurements. Only after recording the first blind comparison may
`--print-reliability-baseline` save `reliability_quality_baseline.json` for the
`--check-reliability` regression gate. Its integer counts preserve existing
failures; passing the gate is not evidence of production readiness. The old
corpus inputs, labels, manifests, and gate baselines remain unchanged.

The first candidate was rejected after losing 13 documented renamed matches.
Its runtime patch, freeze hashes, and raw first comparison are retained. The
accepted new gate uses the original runtime's **before** counts with the same
comparison rules; no reduced-recall candidate floor was adopted.

A second source-blind design was separately frozen before three new cases were
exposed. Their manifest is `reliability_followup_corpus.json`; the first manifest
and its eight cases remain unchanged. Original-runtime counts for the new cases
were appended before the second candidate ran. It changed no new-tranche
decisions but lost one legitimate first-tranche rename and was also rejected.
Both measured experiments are preserved, and the final runtime remains original.
The combined 236-field gate uses only original-runtime counts; neither rejected
candidate lowered an existing floor. The follow-up report separates pooled and
per-tranche results and distinguishes nonregression from an accuracy gain.

This sample targets documented schema migrations and deliberate partial target
projections. It contains publisher examples and schema-only controls, not
production observations or a random sample of real integrations. Domain and
source-kind limitations, missing evidence, deliberate projection choices, and
published-draft status are documented in the corpus README and source bundles.
Independence here concerns annotation and first exposure, not unseen public data
or an independent human adjudication. Results become development feedback after
this first comparison and cannot be reused as a new blind evaluation.
