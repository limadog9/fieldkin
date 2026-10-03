use crate::MatchError;

/// Caller-verified semantic labels. No inference, conversion or synonym lookup
/// is performed. Use a shared vocabulary on both sides (for example `USD` and
/// `kg`); comparison is exact and case-sensitive. Missing labels are absent
/// evidence, and equal labels alone do not establish field equivalence.
///
/// Each supplied label must be nonempty, trimmed and at most 128 UTF-8 bytes.
/// Debug output and built-in reports expose presence/categories, not label values.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct SemanticHints {
    /// Physical or application-defined unit, such as `kg` or `count`.
    pub unit: Option<String>,
    /// Currency vocabulary chosen by the caller, such as `USD` or `EUR`.
    pub currency: Option<String>,
    /// Identifier namespace/scope, such as `tenant:customer` or `catalog:product`.
    pub identifier_scope: Option<String>,
}

impl std::fmt::Debug for SemanticHints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SemanticHints")
            .field("unit", &self.unit.as_ref().map(|_| "<redacted>"))
            .field("currency", &self.currency.as_ref().map(|_| "<redacted>"))
            .field(
                "identifier_scope",
                &self.identifier_scope.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl SemanticHints {
    fn entries(&self) -> [(&'static str, Option<&str>); 3] {
        [
            ("unit", self.unit.as_deref()),
            ("currency", self.currency.as_deref()),
            ("identifier scope", self.identifier_scope.as_deref()),
        ]
    }

    pub(crate) fn validate(&self) -> Result<(), MatchError> {
        for (_, label) in self.entries() {
            if label.is_some_and(|s| s.is_empty() || s.len() > 128 || s.trim() != s) {
                return Err(MatchError(
                    "semantic hints must be nonempty, trimmed labels of at most 128 bytes".into(),
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn compare(&self, target: &Self, warnings: &mut Vec<String>) -> bool {
        let mut conflict = false;
        for ((category, source), (_, target)) in self.entries().into_iter().zip(target.entries()) {
            match (source, target) {
                (Some(source), Some(target)) if source != target => {
                    conflict = true;
                    warnings.push(format!("Caller-supplied {category} hints conflict; pair excluded, no conversion inferred."));
                }
                (Some(_), Some(_)) => warnings.push(format!("Caller-supplied {category} hints agree; this does not establish field equivalence.")),
                (Some(_), None) | (None, Some(_)) => warnings.push(format!("Caller-supplied {category} hint is missing on one side; no agreement inferred.")),
                (None, None) => {}
            }
        }
        conflict
    }
}
