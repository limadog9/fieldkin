# Exact numeric samples

Use `SampleValue::Integer(i128)` for integral samples and
`SampleValue::Decimal(ExactDecimal)` for decimals that must retain exact equality.
Construct these values from the original integer or decimal representation before
any conversion to floating point. Converting an already rounded `f64` cannot
recover the original value.

```rust
use fieldkin::{ExactDecimal, SampleValue};

let identifier = SampleValue::Integer(9_007_199_254_740_993);
let price = SampleValue::Decimal(ExactDecimal::new(1999, 2)?);
assert_eq!(ExactDecimal::new(100, 2)?, ExactDecimal::new(1, 0)?);
assert!(ExactDecimal::new(1, 39).is_err());
# Ok::<(), fieldkin::MatchError>(())
```

`ExactDecimal::new(coefficient, scale)` represents `coefficient * 10^-scale`.
The coefficient accepts the entire signed `i128` range, including `i128::MIN`.
Scale must be between 0 and 38, inclusive. This is an input bound: a larger scale
is rejected even when the coefficient is zero or ends in enough zeroes to reduce
that scale. Construction never rounds or multiplies the coefficient.

Construction removes fractional trailing zeroes. For example, `(100, 2)` and
`(10, 1)` both become `(1, 0)`; every zero becomes `(0, 0)`. The `coefficient()`
and `scale()` accessors expose that canonical representation, so the original
formatting scale is not preserved. Equality and hashing use the canonical value.
No ordering or arithmetic API is provided.

`SampleValue::Number(f64)` remains available for measurements originally stored
as floating point. Exact variants do not turn approximate measurements into exact
facts. Sample overlap keeps integers, decimals, floats and text in separate typed
sets: `Integer(1)`, decimal `1.0`, `Number(1.0)` and text `"1"` do not overlap.
Choose a consistent sample representation for both schemas when comparing them.
Applications also remain responsible for units, currency, time scale and
other semantic differences: the same numeric value does not establish that fields
correspond.

When adapting unsigned integers, use `SampleValue::from_unsigned(value)` to reject
values above `i128::MAX`. An `as i128` cast would wrap those values. Values outside
the supported coefficient or scale bounds need an application-specific matcher
or a deliberate representation policy. Fieldkin does not silently truncate them.

`ExactDecimal` and `SampleValue` redact their `Debug` output. Accessor calls are
explicit access to the sample, so callers should avoid logging their return
values when sample confidentiality matters.

## Scope and dependency choice

This type supports exact sample equality with bounded work and storage. It has
no arithmetic, string parsing, display formatting, rounding or implicit floating
point conversion. Normalization performs at most 38 divisions by ten. It adds
no runtime dependency and does not attempt to replace a decimal arithmetic
library.

The alternative considered was `rust_decimal`. Its 1.43.0 implementation provides
arithmetic and parsing with a 96-bit integer representation and maximum scale 28.
Those are useful capabilities, but Fieldkin needs equality over its full signed
128-bit coefficient range and bounded scale 38, without arithmetic. See the
[crate documentation](https://docs.rs/rust_decimal/1.43.0/rust_decimal/) and
[checked constructor documentation](https://docs.rs/rust_decimal/1.43.0/rust_decimal/struct.Decimal.html#method.try_from_i128_with_scale).
The project uses the [MIT license](https://github.com/paupino/rust-decimal/blob/master/LICENSE).
No code or dependency from that project is included in Fieldkin.

Adding sample enum variants requires downstream exhaustive `match` statements to
handle `Integer` and `Decimal`. Existing callers may keep using `Number`; callers
that need exactness should migrate at the point where values enter the library,
before an integer or decimal is converted to `f64`.
