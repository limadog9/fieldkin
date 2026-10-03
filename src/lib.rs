#![doc = include_str!("../README.md")]

mod assignment;
mod engine;
mod numeric;
mod profile;
mod semantics;
mod signals;
mod types;

pub use engine::{MatchEngine, WeightedMatcher};
pub use numeric::ExactDecimal;
pub use profile::SampleProfileMatcher;
pub use semantics::SemanticHints;
pub use signals::{normalize_name, NameMatcher, SampleMatcher, SampleReliability, TypeMatcher};
pub use types::*;
