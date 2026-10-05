//! Optional caller-configured lexical disagreement checks.

use std::collections::BTreeSet;

use crate::{normalize_name, ConfigurationError, MatchError};

/// Category of a caller-configured name-token conflict.
/// These are heuristic lexical checks, separate from verified semantic hints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NameConflictKind {
    /// Mutually exclusive qualifiers, such as gross and net.
    Qualifier,
    /// Mutually exclusive unit names; no conversion is inferred.
    Unit,
}

/// Mutually exclusive meanings recognized in normalized field-name tokens.
///
/// Each inner vector contains synonymous tokens for one meaning. For example,
/// `[["kg", "kilogram"], ["lb", "pound"]]` recognizes two different units.
/// A pair conflicts only when both names recognize at least one meaning and
/// their recognized meaning sets are disjoint. Missing or overlapping meanings
/// do not establish disagreement. Checks use raw names before matcher aliases;
/// they never increase scores or establish semantic equivalence.
///
/// Each synonym must be a canonical normalized phrase of 1 to 4 tokens and at
/// most 64 UTF-8 bytes, with no duplicate phrase within a rule. Canonical phrases
/// equal `normalize_name(phrase).join(" ")`. Longer matching phrases suppress
/// overlapping shorter matches within a rule, so `"k pa"` does not also count
/// as `"pa"`. Each rule has 2 to 16 alternatives, each containing 1 to 16
/// synonyms. At most 32 rules may be configured.
/// No vocabulary is enabled by default. Caller confirmations can override
/// these lexical exclusions, but not conflicting verified semantic hints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameConflictRule {
    /// Category reported when this rule excludes a pair.
    pub kind: NameConflictKind,
    /// Mutually exclusive meanings, each represented by synonymous token phrases.
    pub alternatives: Vec<Vec<String>>,
}

pub(crate) fn validate(rules: &[NameConflictRule]) -> Result<(), MatchError> {
    let invalid = || MatchError::InvalidConfiguration(ConfigurationError::NameConflicts);
    if rules.len() > 32 {
        return Err(invalid());
    }
    for rule in rules {
        if !(2..=16).contains(&rule.alternatives.len()) {
            return Err(invalid());
        }
        let mut seen = BTreeSet::new();
        for alternatives in &rule.alternatives {
            if !(1..=16).contains(&alternatives.len()) {
                return Err(invalid());
            }
            for token in alternatives {
                if token.len() > 64 {
                    return Err(invalid());
                }
                let normalized = normalize_name(token);
                if !(1..=4).contains(&normalized.len())
                    || normalized.join(" ") != *token
                    || !seen.insert(token)
                {
                    return Err(invalid());
                }
            }
        }
    }
    Ok(())
}

struct Phrase {
    tokens: Vec<String>,
    meaning: u16,
}

pub(crate) struct CompiledNameConflictRule {
    kind: NameConflictKind,
    phrases: Vec<Phrase>,
}

impl CompiledNameConflictRule {
    pub(crate) fn new(rule: &NameConflictRule) -> Self {
        let mut phrases: Vec<_> = rule
            .alternatives
            .iter()
            .enumerate()
            .flat_map(|(index, synonyms)| {
                synonyms.iter().map(move |phrase| Phrase {
                    tokens: phrase.split(' ').map(str::to_owned).collect(),
                    meaning: 1 << index,
                })
            })
            .collect();
        phrases.sort_unstable_by(|a, b| {
            b.tokens
                .len()
                .cmp(&a.tokens.len())
                .then(a.tokens.cmp(&b.tokens))
        });
        Self {
            kind: rule.kind,
            phrases,
        }
    }

    fn meanings(&self, tokens: &[String]) -> u16 {
        let mut longest = vec![0; tokens.len()];
        let mut meanings = 0;
        for phrase in &self.phrases {
            let length = phrase.tokens.len();
            if length > tokens.len() {
                continue;
            }
            for (start, window) in tokens.windows(length).enumerate() {
                let span = &mut longest[start..start + length];
                if window == phrase.tokens && span.iter().all(|covered| *covered <= length) {
                    meanings |= phrase.meaning;
                    span.fill(length);
                }
            }
        }
        meanings
    }
}

pub(crate) struct PreparedNameConflicts {
    // Alternatives are bounded to sixteen, so each field needs one mask per rule.
    meanings: Vec<u16>,
}

impl PreparedNameConflicts {
    pub(crate) fn new(name: &str, rules: &[CompiledNameConflictRule]) -> Self {
        let tokens = normalize_name(name);
        let meanings = rules.iter().map(|rule| rule.meanings(&tokens)).collect();
        Self { meanings }
    }

    pub(crate) fn conflicts_with<'a>(
        &'a self,
        target: &'a Self,
        rules: &'a [CompiledNameConflictRule],
    ) -> impl Iterator<Item = NameConflictKind> + 'a {
        self.meanings
            .iter()
            .zip(&target.meanings)
            .zip(rules)
            .filter(|((source, target), _)| {
                **source != 0 && **target != 0 && **source & **target == 0
            })
            .map(|(_, rule)| rule.kind)
    }
}
