# Valentine port scope and reproducibility

This port targets Valentine 1.0.0 at commit
`d8fa9ee6d312b49ce2c45b62824df1fd31b94a72` from
https://github.com/delftdata/valentine. Matching executes in Rust; reference
Python is development tooling only.

## Coverage

- All five public matcher families and their current algorithm parameters.
- Global batch statistics for COMA instance IDF, distribution ranks, Flooding
  IDF and embedding vocabulary.
- COMA token/trigram names, TF-IDF instances, weighted aggregation and
  bidirectional selection. Current upstream enables NameCM and optional
  InstancesCM; additional complex matchers mentioned in older documentation
  are not enabled in the targeted pipeline.
- Cupid typed tokens, WordNet senses/morphology, Wu-Palmer similarity and
  structural reinforcement. Flat tables use the same database/table/column
  hierarchy as the Python public API.
- Distribution quantile EMD, two-phase discovery, largest-gap cutoffs,
  directed binary triangle-constrained clustering and SHA256 Bloom membership.
  Pure Rust good_lp/microlp replace Python PuLP/CBC.
- Flooding labelled graph products, inverse-average/product policies,
  Basic/A/B/C formulas, convergence and all three string matchers.
- Jaccard six lexical distances, directional fuzzy overlap, Tversky reduction,
  actual cosine embeddings, deduplication and threading.
- Immutable table-aware results, details, filters, Hungarian/greedy/mutual-top
  selection, all six metric classes and configurable extension traits.
- Tables, rows/CSV/JSON, optional native Polars, optional ONNX transformers,
  a matching CLI, offline reference fixtures and cross-platform CI.

## Interface and behavior differences

- Rust Table replaces pandas objects; Rust Polars is optional. Python objects
  and inheritance are replaced by Rust Matcher/Metric extension traits.
- Tables have explicit names without a 26-table limit. match_schemas_with
  supplies aaa/bbb for existing unnamed schemas. Duplicate/empty table column
  names are rejected; the original match_schemas API keeps its old behavior.
- Values are strings. Use consistent numeric formatting when supplying samples.
  CSV expects UTF-8/comma separation; JSON is flat. Type inference checks all
  values for boolean/numeric types; dates are explicit instead of dateutil's
  first-value fuzzy parsing.
- valentine_match takes all supplied values. Explicit MatchOptions sampling is
  evenly spaced per nonempty column and affects every matcher. Python's default
  samples the first 1,000 nonempty rows for COMA instance matching only. Use
  uncapped matching for parity or preselect dataframe rows.
- Empty strings are missing samples for native table matching, including direct
  and batch matcher calls. Python's table adapters can retain empty strings;
  general string and set similarity still treat them as values.
- Rust threads replace multiprocessing and default to one worker.
- Cupid applies Treebank-style tokenization to a label as one sentence. Punkt
  paragraph sentence segmentation is not bundled; identifier/camel/snake-case,
  symbol and contraction behavior is tested against upstream.
- Cupid uses deterministic WordNet synset ordering. Upstream sorts synset pairs
  by Python object addresses before calling NLTK Wu-Palmer similarity; some
  sense pairs have different forward/reverse scores, so upstream scores can
  change across processes or matching histories. The optional reference source
  fix orders pairs by canonical WordNet names, matching Rust's bundled index.
- COMA uses f64 instead of float32 instance matrices, so reference comparisons
  allow small rounding differences. Flooding/Distribution use tight f64
  tolerances. Equal-optimum integer clusters may differ between microlp and CBC.
- Distribution preserves original column identifiers. Upstream decodes
  identifiers from PuLP variable names, which can replace punctuation such as
  square brackets with underscores. The optional reference source fix gives
  variables unique numeric IDs and retrieves column keys from the original
  dictionary, avoiding sanitization and collisions.
- Jaccard preserves RapidFuzz's float32 scorer cutoff conversion for lexical
  fuzzy matching. Exact and cosine modes do not use that conversion.
- Equal-score Hungarian/greedy results always remain one-to-one. Upstream's
  equal-score shortcut returns all matches. All selectors use deterministic
  tie ordering. Hungarian maximizes total score before threshold filtering.
- Metrics treat gold pairs and name-only predictions as sets. GroundTruth::Names
  is intended for one table pair; use Columns for table-aware batch evaluation.
  MRR measures the first correct target's rank within each source.
  METRICS_ALL gives raw precision/recall/F1 distinct WithoutOneToOne keys so
  both configurations remain visible rather than overwriting one another.
- Embeddings use FastEmbed ONNX exports or a supplied provider. MiniLM is not
  promised bit-for-bit identical to PyTorch. Model/cache/device settings are
  native options; CUDA/MPS availability follows the ONNX execution provider.
- Invalid inputs/configurations return typed errors. Scores must be finite
  and in [0,1]; percentage/configuration bounds are checked.
- The original research experiment suite on Valentine's old branch is distinct
  from the current library. This repository keeps Fieldkin evaluation corpora
  and adds fixtures/CLI tooling rather than vendoring notebooks or website code.

## Regenerate fixtures

Python 3.10+ and reference dependencies are needed only for regeneration:

```sh
git clone https://github.com/delftdata/valentine .upstream/valentine
git -C .upstream/valentine checkout d8fa9ee6d312b49ce2c45b62824df1fd31b94a72
python -m pip install --target .upstream/python numpy pandas scipy networkx nltk rapidfuzz 'pulp>=2.5,<4.0' POT anytree chardet python-dateutil defusedxml
python scripts/generate_jaccard_fixtures.py
python scripts/generate_distribution_flooding_fixtures.py
python scripts/generate_coma_cupid_fixtures.py
```

The Cupid generator needs NLTK WordNet, stopwords and punkt_tab in
.upstream/nltk_data. Download them with NLTK before regeneration.
scripts/build_wordnet.py rebuilds the compact runtime corpus from the
checksum-pinned Princeton archive; its command-line help and source record
the input checksum. assets/WORDNET-LICENSE is included unchanged.

Standard Rust tests read checked-in JSON, with no Python, NLTK data download,
external solver executable or model download. The optional ignored model test
explicitly exercises native ONNX inference with downloaded MiniLM weights.

## Compare the evaluation datasets live

After preparing the same reference dependencies and NLTK corpora above, run:

```sh
cargo run --locked --example valentine_parity
python scripts/run_valentine_parity.py
python scripts/compare_valentine_parity.py
```

Both runners use every checked-in JSON dataset in `eval` and `eval_realworld`,
preserving column names, sample order, duplicates and numeric string formatting.
Declared types are mapped to the same `int`, `float`, `date` and `varchar`
categories before matching; independent type inference is outside this check.
The comparison checks input SHA256 hashes, complete column-pair sets and scores
for all five default matchers plus COMA with instance matching enabled. It
records exact equality separately from numerical tolerances (`1e-6` for COMA,
`1e-10` for the other matchers), and exits unsuccessfully on differences or
incomplete runs. Results are written to `target/parity`.

The default Python run uses the unmodified upstream commit. To resolve the two
upstream defects above, compare with an explicitly patched source copy:

```sh
cargo run --locked --example valentine_parity -- target/parity/rust_results_resolved.json
python scripts/run_valentine_parity.py --reference-fixes
python scripts/compare_valentine_parity.py --rust target/parity/rust_results_resolved.json --python target/parity/python_results_resolved.json --output target/parity/comparison_resolved.json
python scripts/test_distribution_reference_patch.py
```

The two auditable source patches are
[`cupid.patch`](scripts/reference_patches/cupid.patch) and
[`distribution.patch`](scripts/reference_patches/distribution.patch).
`scripts/valentine_reference.py` verifies the pinned, clean checkout and applies
them to a separate package under `target/parity/reference`. The run records
patch hashes and the original/patched source hashes. The comparator checks
provenance and requires every returned identifier to exist in the input schema;
it retains the original score tolerances and compares raw outputs.

The original `python_results.json` and `comparison.json` remain the evidence for
unmodified upstream behavior. The `_resolved.json` files report the corrected
reference separately. Rust's matching behavior already implements both fixes;
the released library does not need a runtime behavior change for these cases.

The checked-in [validation summary](validation/valentine_parity.json) records a
full corrected-reference run: 90/90 comparisons pass, with 60 numerically exact
cases and 30 within the unchanged rounding tolerances. It includes every input
hash and both source patch hashes; full pair-by-pair reports stay in `target`.

To exercise Cupid in three fresh processes with different lookup orders, use
the `reference.source_root` path from `python_results_resolved.json`:

```sh
python scripts/test_cupid_reference_fix.py --reference-dir PATH_TO_PATCHED_REFERENCE
```

Rust regression tests cover canonical WordNet ties, changed Cupid weights and
worker counts, and distinct column names that would collide after solver
sanitization. Python regressions check the actual patched matcher and solver.
