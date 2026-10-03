use std::fmt;

use crate::MatchError;

/// An exact decimal sample represented by `coefficient * 10^-scale`.
///
/// The coefficient spans all of [`i128`], and scale is limited to `0..=38`.
/// Construction removes fractional trailing zeroes, so `1.0` and `1.00` compare
/// equal and hash identically. Zero always has coefficient and scale zero.
///
/// This is an equality key for sample evidence, not an arithmetic type. Numeric
/// ordering, rounding, parsing, float conversion, and arithmetic are not provided. `Debug`
/// redacts the value; the explicit accessors return its canonical representation.
///
/// ```
/// use fieldkin::ExactDecimal;
///
/// assert_eq!(ExactDecimal::new(100, 2)?, ExactDecimal::new(1, 0)?);
/// assert_ne!(ExactDecimal::new(1, 38)?, ExactDecimal::new(2, 38)?);
/// assert!(ExactDecimal::new(0, 39).is_err());
/// # Ok::<(), fieldkin::MatchError>(())
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExactDecimal {
    coefficient: i128,
    scale: u8,
}

impl ExactDecimal {
    /// Constructs an exact decimal without rounding or overflow.
    ///
    /// Returns an error if `scale > 38`, including for zero or coefficients whose
    /// trailing zeroes would otherwise reduce the scale. The bound is checked
    /// before normalization, and the error never contains the sample value.
    pub fn new(mut coefficient: i128, scale: u32) -> Result<Self, MatchError> {
        if scale > 38 {
            return Err(MatchError("exact decimal scale must be at most 38".into()));
        }
        let mut scale = scale as u8;
        if coefficient == 0 {
            scale = 0;
        } else {
            while scale > 0 && coefficient % 10 == 0 {
                coefficient /= 10;
                scale -= 1;
            }
        }
        Ok(Self { coefficient, scale })
    }

    /// Returns the canonical coefficient, with fractional trailing zeroes removed.
    pub fn coefficient(self) -> i128 {
        self.coefficient
    }

    /// Returns the canonical scale in `0..=38`.
    pub fn scale(self) -> u8 {
        self.scale
    }
}

impl fmt::Debug for ExactDecimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ExactDecimal(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use proptest::prelude::*;

    use super::ExactDecimal;

    #[test]
    fn retains_full_signed_coefficient_range() {
        for coefficient in [i128::MIN, i128::MAX] {
            for scale in [0, 1, 38] {
                let value = ExactDecimal::new(coefficient, scale).unwrap();
                assert_eq!(value.coefficient(), coefficient);
                assert_eq!(u32::from(value.scale()), scale);
            }
        }
    }

    #[test]
    fn rejects_excess_scale_before_normalizing() {
        for coefficient in [0, 10, -100, i128::MIN, i128::MAX] {
            for scale in [39, u32::MAX] {
                let error = ExactDecimal::new(coefficient, scale).unwrap_err();
                assert_eq!(error.0, "exact decimal scale must be at most 38");
            }
        }
        assert!(ExactDecimal::new(1, 38).is_ok());
    }

    #[test]
    fn equivalent_representations_share_equality_and_keys() {
        let values = [
            ExactDecimal::new(1, 0).unwrap(),
            ExactDecimal::new(10, 1).unwrap(),
            ExactDecimal::new(100, 2).unwrap(),
        ];
        assert_eq!(values[0], values[1]);
        assert_eq!(values[1], values[2]);
        assert_eq!(values.into_iter().collect::<HashSet<_>>().len(), 1);
        let negative = ExactDecimal::new(-123400, 4).unwrap();
        assert_eq!((negative.coefficient(), negative.scale()), (-1234, 2));
    }

    #[test]
    fn adjacent_exact_values_remain_distinct() {
        assert_ne!(
            ExactDecimal::new(9_007_199_254_740_992, 0).unwrap(),
            ExactDecimal::new(9_007_199_254_740_993, 0).unwrap()
        );
        assert_ne!(
            ExactDecimal::new(1, 38).unwrap(),
            ExactDecimal::new(2, 38).unwrap()
        );
        assert_ne!(
            ExactDecimal::new(i128::MAX, 38).unwrap(),
            ExactDecimal::new(i128::MAX - 1, 38).unwrap()
        );
    }

    #[test]
    fn canonical_zero_and_integral_trailing_zeroes() {
        for scale in 0..=38 {
            let zero = ExactDecimal::new(0, scale).unwrap();
            assert_eq!((zero.coefficient(), zero.scale()), (0, 0));
        }
        let integral = ExactDecimal::new(1000, 2).unwrap();
        assert_eq!((integral.coefficient(), integral.scale()), (10, 0));
    }

    #[test]
    fn debug_redacts_values_including_alternate_format() {
        let value = ExactDecimal::new(9384756, 4).unwrap();
        assert_eq!(format!("{value:?}"), "ExactDecimal(<redacted>)");
        assert_eq!(format!("{value:#?}"), "ExactDecimal(<redacted>)");
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn canonical_equality_matches_exact_cross_multiplication(
            left_coefficient in -1_000_000_000_000_i128..=1_000_000_000_000_i128,
            right_coefficient in -1_000_000_000_000_i128..=1_000_000_000_000_i128,
            left_scale in 0_u32..=18,
            right_scale in 0_u32..=18,
        ) {
            // Every product is bounded by 10^30, below i128::MAX. This oracle
            // compares rational values directly instead of copying normalization.
            let same_value = left_coefficient * 10_i128.pow(right_scale)
                == right_coefficient * 10_i128.pow(left_scale);
            let left = ExactDecimal::new(left_coefficient, left_scale).unwrap();
            let right = ExactDecimal::new(right_coefficient, right_scale).unwrap();
            prop_assert_eq!(left == right, same_value);
        }

        #[test]
        fn rescaling_preserves_value_and_canonical_form(
            coefficient in -1_000_000_000_000_i128..=1_000_000_000_000_i128,
            scale in 0_u32..=20,
            extra_scale in 0_u32..=18,
        ) {
            let original = ExactDecimal::new(coefficient, scale).unwrap();
            let rescaled = ExactDecimal::new(
                coefficient * 10_i128.pow(extra_scale), scale + extra_scale
            ).unwrap();
            prop_assert_eq!(original, rescaled);
            prop_assert_eq!(
                ExactDecimal::new(original.coefficient(), u32::from(original.scale())).unwrap(),
                original
            );
            prop_assert!(original.scale() == 0 || original.coefficient() % 10 != 0);
        }
    }
}
