# Bounded Valentine comparison protocol

This experiment compares Fieldkin with the COMA implementation supplied by
Valentine 1.0.0 on the fixed Northix fixture. It is a development diagnostic over
84 table pairs, not a reproduction of a COMA or Valentine research paper, an
independent production validation, or evidence that either library is generally
more accurate. The public upstream dataset is synthetic. No T2D reserved classes
are involved.

The [retained-runtime results](../evaluation/results/continuation-v3/northix/artifacts/results.md)
and [failure analysis](northix-evaluation.md#recorded-results) use the original
frozen score exports: [546 schema-only pairs](../evaluation/results/continuation-v1/valentine/valentine-schema_only.json)
and [1,557 schema-and-samples pairs](../evaluation/results/continuation-v1/valentine/valentine-schema_and_samples.json).
Each file covers all 84 tasks exactly once. Schema-only COMA yields 17 correct
proposals out of 31 independently or 27 under one-to-one selection, with 60.71%
positive-field recall. COMA with samples yields no proposals: its maximum raw
score, 0.6739781945943832, is below the shared 0.70 cutoff. This illustrates the
score-comparability limitation rather than establishing a model ranking.
The [v1 comparator run](../evaluation/results/continuation-v1/valentine/run.json)
was not repeated when the Rust runtime was requalified; the
[v3 Rust execution](../evaluation/results/continuation-v3/northix/run.json)
records the same input score hashes and unchanged matching outcomes.

## Inputs and frozen matcher settings

The [comparison tool](../evaluation/compare_valentine.py) reads only the fixture's
`tables` and `pairs` as matching inputs. It deliberately discards other table and
field metadata, and never supplies class labels to the matcher. Field IDs remain
the original complete input identifiers; original column names are retained.
Table names are replaced with the neutral names `aaa` and `bbb`, so database and
table identity do not provide extra matching evidence through schema paths.

Each field becomes an explicitly `object`-typed pandas Series. Strings are kept
as strings and nulls remain missing. Short sampled columns are padded with nulls
to 64 rows. The importer selected these bounded samples before matching; the
wrapper neither resamples nor consults labels. `instance_sample_size=None`
disables Valentine's additional row sampling.

Two fixed modes are run over all 84 pairs:

| Setting | Schema only | Schema and samples |
| --- | --- | --- |
| Algorithm | `Coma` from Valentine 1.0.0 | Same |
| `use_schema` | `true` | `true` |
| `use_instances` | `false` | `true` |
| `max_n` | `0` (unlimited) | Same |
| `delta` | `1.0` | Same |
| `threshold` | `0.0` | Same |
| `instance_weight` | `1.0` | Same |
| DataFrame cells | 64 nulls per column | Fixture strings/nulls padded to 64 rows |

The wrapper exports every returned finite score in `[0,1]`, including zero if
returned. It does not use Valentine's convenience metrics or assignment helpers,
filter to a top-k list, calibrate scores, manufacture missing edges, or substitute
class labels for evidence. Unexpected identities, duplicate pairs or invalid
scores fail the run. Column names map back to stable field IDs before export.

## Common selector and comparison limits

The Rust evaluator consumes the exported scores through the same fixed 0.70
eligibility threshold and 0.08 ambiguity margin, with the same abstention and
independent/partial-one-to-one policies as the Fieldkin comparison. This holds
downstream selection rules constant. It does **not** make the models' score scales
equivalent or calibrated probabilities. A lower proposal rate can reflect score
scale rather than better caution, and a higher rate can reflect scale rather than
better evidence. No per-model threshold tuning is performed in this experiment.

Fieldkin receives these imported sample strings as `SampleValue::Text`; its
sample signal compares exact distinct values. Valentine's native instance path
drops missing/empty values and applies its own tokenization, English-stopword
handling and TF-IDF similarity. Those are materially different representations.
The wrapper preserves the bounded input strings and lets each implementation
apply its documented processing; equal samples do not mean equal features.

COMA's structural/schema components and instance combination also differ from
Fieldkin's weighted name/type/sample signals. This diagnostic does not isolate
individual algorithms or establish a fair universal ranking. The protocol and
results must retain per-mode proposal and abstention counts, label limitations
and the fixed selector settings when reporting comparisons.

## Reproducible local execution

The [environment record](../evaluation/valentine-environment.json) pins the
recorded CPython 3.11.4 Windows environment, 21 resolved packages, bootstrap tools,
wheel URLs/hashes and separately provisioned stopword resources. The
[requirements file](../evaluation/valentine-requirements.txt) records the pinned
wheel hashes. Python dependencies are isolated experiment tools; they are not
dependencies of the Rust library. Dependency and dataset attribution are recorded
separately from Fieldkin's MIT OR Apache-2.0 code license.

`prepare` records raw-byte hashes of the fixture, Northix protocol, wrapper,
requirements and environment record; Python executable/base-executable identity;
installed distribution versions and non-pyc files; and local resource files.
Installed versions must match the locked package/bootstrap inventory, and the
stopword files must match their frozen hashes. `run` requires the same context
before and after both modes. Added or changed bytecode caches are excluded;
ordinary source, extension and metadata files in each distribution are included.

The expected local stopword resource is
`.local/nltk_data/corpora/stopwords/english`. The tool restricts `nltk.data.path`
to the supplied workspace directory, validates English stopwords before scoring,
disables NLTK/Valentine automatic downloading, and blocks ordinary Python socket
connections and URL download entry points during matching. This prevents the
normal fallback from downloading corpora into the user's home directory. It is a
guard against unintended I/O, not a hostile-code security sandbox.

Freeze all matching inputs, protocol, wrapper and environment records before
preparation or scoring. Provisioning dependencies/resources is a separate step.
Once the pinned local environment exists, the commands are:

```powershell
$env:PYTHONHASHSEED = "0"
$env:OMP_NUM_THREADS = "1"
$env:OPENBLAS_NUM_THREADS = "1"
$env:MKL_NUM_THREADS = "1"
.local/valentine-env/Scripts/python.exe evaluation/compare_valentine.py prepare --nltk-data .local/nltk_data --output evaluation/results/valentine-v1
.local/valentine-env/Scripts/python.exe evaluation/compare_valentine.py run --nltk-data .local/nltk_data --output evaluation/results/valentine-v1
```

The destination must be fresh at preparation and contain only `prepare.json`
when running. The tool writes `valentine-schema_only.json` and
`valentine-schema_and_samples.json`, each covering the 84 fixed pair IDs exactly
once. A final `run.json` records success and both artifact hashes only after both
modes complete and the context still matches. Partial results remain for
inspection without a success record; they cannot be overwritten by rerunning.
All JSON uses explicit LF newlines and rejects nonfinite numbers.

Third-party output and exception details are suppressed because they could expose
sample values. Output artifacts contain stable identities and scores, not samples.
The test command below uses only the standard library, mock contexts and tiny
synthetic fixtures. It never imports Valentine or scores the Northix corpus:

```sh
python -m unittest discover -s evaluation -p test_compare_valentine.py
```

These records detect accidental local drift and assume the wrapper, installed
packages, interpreter, standard library and local processes are trusted. They do
not constitute a signed attestation, capture every native dependency or provide
a hermetic build. Different platforms require a newly recorded environment and
result version; the pinned Windows wheel list is not a portable lockfile.
