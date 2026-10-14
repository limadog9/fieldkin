#![doc = include_str!("../README.md")]

pub mod algorithms;
#[cfg(feature = "embeddings")]
pub mod embeddings;
mod engine;
mod limits;
pub mod metrics;
#[cfg(feature = "polars")]
mod polars;
mod results;
mod signals;
mod synonyms;
mod table;
mod types;

pub use algorithms::{
    CheckedMatcher, Matcher, match_schemas_with, match_tables, match_tables_one_to_one_with_limits,
    valentine_match,
};
pub use engine::{Config, match_schemas, try_match_schemas};
pub use limits::WorkLimits;
pub use results::{ColumnPair, MatchDetails, MatcherResults};
pub use table::{Error, MatchOptions, Table};
pub use types::{Candidate, DataType, Decision, Field, FieldResult, MatchReport, Schema};
