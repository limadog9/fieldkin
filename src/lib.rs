#![doc = include_str!("../README.md")]

mod assignment;
mod engine;
mod signals;
mod types;

pub use engine::{MatchEngine, WeightedMatcher};
pub use signals::{normalize_name, NameMatcher, SampleMatcher, TypeMatcher};
pub use types::*;
