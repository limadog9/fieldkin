mod engine;
mod signals;
mod types;
mod synonyms;

pub use engine::{match_schemas, Config};
pub use types::{
    Candidate, DataType, Decision, Field, FieldResult, MatchReport, Schema,
};
