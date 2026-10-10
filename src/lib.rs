#![doc = include_str!("../README.md")]

pub mod algorithms;
#[cfg(feature = "embeddings")]
pub mod embeddings;
mod engine;
pub mod metrics;
#[cfg(feature = "polars")]
mod polars;
mod results;
mod signals;
mod synonyms;
mod table;
mod types;

pub use algorithms::{Matcher, match_schemas_with, match_tables, valentine_match};
pub use engine::{Config, match_schemas, try_match_schemas};
pub use results::{ColumnPair, MatchDetails, MatcherResults};
pub use table::{Error, MatchOptions, Table};
pub use types::{Candidate, DataType, Decision, Field, FieldResult, MatchReport, Schema};
