#![doc = include_str!("../README.md")]

mod assignment;
mod constraints;
mod diagnostics;
mod engine;
mod error;
mod numeric;
mod profile;
mod semantics;
mod signals;
mod types;

/// Optional, bounded JSON export and application-owned review persistence.
#[cfg(feature = "json")]
pub mod json;

pub use constraints::{FieldPair, MatchConstraints};
pub use diagnostics::*;
pub use engine::{MatchEngine, WeightedMatcher};
pub use error::*;
pub use numeric::ExactDecimal;
pub use profile::SampleProfileMatcher;
pub use semantics::SemanticHints;
pub use signals::{normalize_name, NameMatcher, SampleMatcher, SampleReliability, TypeMatcher};
pub use types::*;
