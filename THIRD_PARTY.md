# Third-party acknowledgments

Fieldkin's original code and synthetic fixtures are copyright 2026 Fieldkin contributors, licensed under MIT OR Apache-2.0. [@limadog9](https://github.com/limadog9) is the sole maintainer and final decision-maker. Dependency code retains its own copyright and license.

## Dependencies

The [native Rust dependency review](qualification/results/rust-native-v1/dependencies.json)
inventories all 72 locked third-party versions across five manifest graphs,
including advisory results and license-file hashes. All three lockfile
scans reported zero vulnerabilities against the freshly fetched RustSec
database at `ef6173cbc5c50ec8166f9a5b28f07834144373ee`;
this is a dated advisory check, not a source-code security audit. The
[earlier review and interpretation](docs/dependency-review.md) retain the
October 3 observations, the additional Unicode-3.0 notice for `unicode-ident`,
and the missing standalone notice file in the optional `stats_alloc` package.
The isolated qualification executable adds no dependency beyond Fieldkin itself.
The unpublished Rust tooling crate adds CSV and bounded ZIP/gzip/tar importers,
TOML configuration parsing and provenance hashing. These are development tools
and are excluded from the published library dependency graph. The review records
their complete dependency licenses and available notice hashes. Existing dated
reviews remain archived. The former Rust 1.85 development pin for proptest has
been removed; the project now uses current stable Rust and proptest 1.11.

The default library feature set resolves these runtime crates:

| Crate | Version | License expression | Upstream |
| --- | --- | --- | --- |
| strsim | 0.11.1 | MIT | [rapidfuzz/strsim-rs](https://github.com/rapidfuzz/strsim-rs) |
| unicode-normalization | 0.1.25 | MIT OR Apache-2.0 | [unicode-rs/unicode-normalization](https://github.com/unicode-rs/unicode-normalization) |
| tinyvec (transitive) | 1.13.3 | Zlib OR Apache-2.0 OR MIT | [Lokathor/tinyvec](https://github.com/Lokathor/tinyvec) |

The optional `json` feature additionally uses these direct dependencies, at the
versions already present in the evaluator's reviewed lockfile:

| Crate | Version | License expression | Upstream |
| --- | --- | --- | --- |
| serde | 1.0.229 | MIT OR Apache-2.0 | [serde-rs/serde](https://github.com/serde-rs/serde) |
| serde_json | 1.0.151 | MIT OR Apache-2.0 | [serde-rs/json](https://github.com/serde-rs/json) |

Their enabled runtime dependencies include `serde_core`, `itoa`, `memchr` and
`zmij`. Serde's derive support uses `serde_derive`, `proc-macro2`, `quote`, `syn`
and `unicode-ident` during compilation. The full version/license/notice inventory
is in the new review; in particular, `unicode-ident` requires its Unicode-3.0
notice in addition to an MIT or Apache-2.0 choice. These feature-specific edges
do not change the default library dependency graph. No locked versions were
upgraded to add JSON support.

[`proptest` 1.11.0](https://github.com/proptest-rs/proptest/tree/v1.11.0) is a development-only direct dependency, licensed MIT OR Apache-2.0, copyright 2016 FullContact, Inc. Its transitive test dependencies are recorded in `Cargo.lock` and retain their upstream licenses. They are not dependencies of consumers of the Fieldkin library. Cargo registry packages supply their own license files; preserve the relevant notices when redistributing bundled dependency code. The runtime MIT notices are reproduced below for convenience.

## Research acknowledgment

Koutras et al., *Valentine: Evaluating Matching Techniques for Dataset Discovery*, ICDE 2021, pp. 468–479 ([paper](https://arxiv.org/abs/2010.07386), [repository](https://github.com/delftdata/valentine)), informed the landscape review. Valentine is Apache-2.0, copyright 2021–2026 Delft University of Technology at inspection. No Valentine source code, datasets, or prose is included. Fieldkin's matching and assignment implementation is original and uses general techniques from schema matching, string similarity, and bipartite assignment. See [the landscape note](docs/landscape.md) for scope and comparison limits.

The development-only `evaluation/external/t2d-v1/` directory contains original
T2D correspondence annotations by Dominique Ritze, Oliver Lehmberg and Christian
Bizer, and a derived annotation-only schema task. The source page explicitly
licenses [correspondences under Apache-2.0](https://webdatacommons.org/webtables/goldstandard.html);
the separate terms for web tables and DBpedia values are not used as permission
to redistribute those data, which are not included. The directory supplies a
license text, attribution/modification NOTICE and exact source hashes. These data
are excluded from the library package. Unlabeled alternatives remain unknown;
the induced task is not the full T2D benchmark or independent consumer validation.

Library examples, tests and performance inputs remain original synthetic data
under Fieldkin's MIT OR Apache-2.0 terms. External evaluation data retain the
specific license and provenance recorded alongside them.

The development-only Northix archive and derived fixture in
[`evaluation/external/northix-v1/`](evaluation/external/northix-v1/) and
[`evaluation/fixtures/northix-v1.json`](evaluation/fixtures/northix-v1.json) are
by Farid Bourennani (2012), published by the UCI Machine Learning Repository,
DOI [10.24432/C5M60J](https://doi.org/10.24432/C5M60J), under
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). The directory includes
the full license, source-license evidence, source hashes and a modification NOTICE.
The original archive remains unchanged; the derived fixture groups original
columns, selects up to 64 InputData records per column, represents blank records
as null and retains class labels separately. Fifteen differing class-file copies
are recorded rather than used as sample evidence. These data are excluded from
the Rust library package and retain their own license. No creator or publisher
endorsement is implied. See the [task and limitations](docs/northix-evaluation.md).

The optional development comparison executes Apache-2.0
[Valentine 1.0.0](https://github.com/delftdata/valentine), copyright Delft
University of Technology, from a separate local Python environment. Its wheel
and transitive dependencies are pinned by hash in
[`evaluation/valentine-requirements.txt`](evaluation/valentine-requirements.txt);
[`evaluation/valentine-environment.json`](evaluation/valentine-environment.json)
records the installed versions, artifact hashes and local NLTK stopword inputs.
Valentine code, Python dependencies and stopword data are not vendored or included
in the Rust library package. Their own notices accompany the installed packages.
The [comparison guide](docs/valentine-comparison.md) distinguishes external raw
scores from Fieldkin's selection policy. The Rust dependency audit does not audit
this separate Python environment.

The unpublished `evaluation/` workspace package uses `serde` (MIT OR Apache-2.0),
`serde_json` (MIT OR Apache-2.0), and `sha2` (MIT OR Apache-2.0) for development-only
artifacts and provenance hashes. Only the optional library `json` feature adds
Serde and serde_json to the consumer graph; `sha2` remains evaluator-only. Exact
versions and transitive packages are frozen in `Cargo.lock` and copied into
evaluation result metadata. The 40 synthetic evaluation families are original
Fieldkin content; the separately attributed external fixtures above are not.

The unpublished, isolated `performance/` package optionally uses
[`stats_alloc` 0.1.10](https://github.com/neoeinstein/stats_alloc), licensed MIT,
for allocation instrumentation in benchmark executables. It is absent from the
library's dependency graph and ordinary timing builds. Its exact version and
license accompany the package in the Cargo registry; the performance lockfile
records the separate dependency graph. No dependency source is vendored here.

## Runtime dependency notices


### strsim

```text
The MIT License (MIT)

Copyright (c) 2015 Danny Guo
Copyright (c) 2016 Titus Wormer <tituswormer@gmail.com>
Copyright (c) 2018 Akash Kurdekar

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### unicode-normalization

```text
Copyright (c) 2015 The Rust Project Developers

Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
```

### tinyvec

```text
Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```
