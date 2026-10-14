//! Exact-output verification shared by the opt-in performance examples.

use fieldkin::MatcherResults;
use sha2::{Digest, Sha256};

pub fn results_digest(results: &MatcherResults) -> Result<String, Box<dyn std::error::Error>> {
    let mut hash = Sha256::new();
    let mut append = |selected: &MatcherResults| append_results(&mut hash, selected);
    // Release each verification result before constructing the next one.
    append(results)?;
    append(&results.filter(0.5)?)?;
    append(&results.take_top_n(10))?;
    append(&results.one_to_one_greedy(Some(0.5))?)?;
    append(&results.one_to_one_hungarian(Some(0.5))?)?;
    append(&results.one_to_one_hungarian_threshold_aware(Some(0.5))?)?;
    Ok(format!("{:x}", hash.finalize()))
}

/// Hash only the supplied order, identities, score bits, and component details.
/// Unlike `results_digest`, this does not execute additional assignment solves.
#[allow(dead_code)] // The parity example imports only the combined digest.
pub fn ordered_results_digest(
    results: &MatcherResults,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut hash = Sha256::new();
    append_results(&mut hash, results)?;
    Ok(format!("{:x}", hash.finalize()))
}

fn append_results(
    hash: &mut Sha256,
    selected: &MatcherResults,
) -> Result<(), Box<dyn std::error::Error>> {
    // Preserve iteration order and IEEE-754 bits, including component scores.
    let entries: Vec<_> = selected
        .iter()
        .map(|(pair, score)| {
            (
                pair,
                score.to_bits(),
                selected.get_details(pair).map(|details| {
                    details
                        .iter()
                        .map(|(name, value)| (name, value.to_bits()))
                        .collect::<Vec<_>>()
                }),
            )
        })
        .collect();
    let bytes = serde_json::to_vec(&entries)?;
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    Ok(())
}
