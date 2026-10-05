#![doc = include_str!("../README.md")]

mod assignment;
mod constraints;
mod contextual;
mod diagnostics;
mod engine;
mod error;
mod name_conflicts;
mod numeric;
mod profile;
mod semantic_name;
mod semantics;
mod signals;
mod types;

/// Optional, bounded JSON export and application-owned review persistence.
#[cfg(feature = "json")]
pub mod json;

pub use constraints::{FieldPair, MatchConstraints};
pub use contextual::ContextualEvidence;
pub use diagnostics::*;
pub use engine::{MatchEngine, WeightedMatcher};
pub use error::*;
pub use name_conflicts::{NameConflictKind, NameConflictRule};
pub use numeric::ExactDecimal;
pub use profile::SampleProfileMatcher;
pub use semantics::SemanticHints;
pub use signals::{normalize_name, NameMatcher, SampleMatcher, SampleReliability, TypeMatcher};
pub use types::*;
