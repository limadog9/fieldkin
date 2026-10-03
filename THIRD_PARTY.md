# Third-party acknowledgments

Fieldkin's original code and synthetic fixtures are copyright 2026 Fieldkin contributors, licensed under MIT OR Apache-2.0. [@limadog9](https://github.com/limadog9) is the sole maintainer and final decision-maker. Dependency code retains its own copyright and license.

## Dependencies

The reviewed lockfile resolves these runtime crates:

| Crate | Version | License expression | Upstream |
| --- | --- | --- | --- |
| strsim | 0.11.1 | MIT | [rapidfuzz/strsim-rs](https://github.com/rapidfuzz/strsim-rs) |
| unicode-normalization | 0.1.25 | MIT OR Apache-2.0 | [unicode-rs/unicode-normalization](https://github.com/unicode-rs/unicode-normalization) |
| tinyvec (transitive) | 1.13.3 | Zlib OR Apache-2.0 OR MIT | [Lokathor/tinyvec](https://github.com/Lokathor/tinyvec) |

[`proptest` 1.6.0](https://github.com/proptest-rs/proptest/tree/v1.6.0) is a development-only direct dependency, licensed MIT OR Apache-2.0, copyright 2016 FullContact, Inc. Its transitive test dependencies are recorded in `Cargo.lock` and retain their upstream licenses. They are not dependencies of consumers of the Fieldkin library. Cargo registry packages supply their own license files; preserve the relevant notices when redistributing bundled dependency code. The runtime MIT notices are reproduced below for convenience.

## Research acknowledgment

Koutras et al., *Valentine: Evaluating Matching Techniques for Dataset Discovery*, ICDE 2021, pp. 468–479 ([paper](https://arxiv.org/abs/2010.07386), [repository](https://github.com/delftdata/valentine)), informed the landscape review. Valentine is Apache-2.0, copyright 2021–2026 Delft University of Technology at inspection. No Valentine source code, datasets, or prose is included. Fieldkin's matching and assignment implementation is original and uses general techniques from schema matching, string similarity, and bipartite assignment. See [the landscape note](docs/landscape.md) for scope and comparison limits.

No third-party datasets are bundled. The examples, test fixtures, and benchmark inputs are synthetic and distributed under Fieldkin's MIT OR Apache-2.0 terms.

The unpublished `evaluation/` workspace package uses `serde` (MIT OR Apache-2.0),
`serde_json` (MIT OR Apache-2.0), and `sha2` (MIT OR Apache-2.0) for development-only
artifacts and provenance hashes. These are not runtime dependencies of the Fieldkin
library. Their exact versions and transitive packages are frozen in `Cargo.lock`
and copied into evaluation result metadata. No third-party fixture data is used;
all 40 evaluation families are original synthetic Fieldkin content.

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
