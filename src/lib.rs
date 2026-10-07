#![doc = include_str!("../README.md")]

mod engine;
mod signals;
mod synonyms;
mod types;

pub use engine::{Config, match_schemas};
pub use types::{Candidate, DataType, Decision, Field, FieldResult, MatchReport, Schema};
