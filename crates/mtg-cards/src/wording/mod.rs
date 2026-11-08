//! Wording-only differences: whether a clause the engine writes back as other text than the card (`Differs`) says the
//! same thing under a fixed, listed set of equivalences, so a player can approve such cards in bulk
//! (`docs/cards-dsl.md`, Read tier, Bulk reads).
//!
//! Each equivalence is a named [`Rule`] that rewrites both wordings to one canonical form; a clause matches when the
//! two canonical forms are equal. A rule never drops a number, a target, a controller, a duration, a "may" or a
//! condition, so a clause that differs in one of those never matches.

mod rules;

use serde::{Deserialize, Serialize};

pub use rules::Rule;

use crate::proposal::{ClauseReport, ClauseState};

/// One `Differs` clause that matches its printed words, with the rules it needed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClauseMatch {
    /// The face of the clause.
    pub face: usize,
    /// The clause as printed (the card's name as `~`).
    pub printed: String,
    /// The engine's words for it.
    pub written: String,
    /// The equivalences that make the two the same, in the order of [`Rule::ALL`]; never empty.
    pub rules: Vec<Rule>,
}

/// The rules that make `written` say what `printed` says, the fewest needed; `None` when the two differ in substance.
pub fn same_words(printed: &str, written: &str) -> Option<Vec<Rule>> {
    let used = rules::matching(printed, written, Rule::ALL)?;
    // Drop a rule the match holds without, so a card lists only what a reader must check.
    let mut needed = used.clone();
    for rule in &used {
        let without: Vec<Rule> = needed.iter().copied().filter(|r| r != rule).collect();
        if rules::matching(printed, written, &without).is_some() {
            needed = without;
        }
    }
    Some(needed)
}

/// The bulk check of a card: every clause it reads back as other text matched, or `None` when one is not (or none
/// differs, since such a card needs no read for its wording).
pub fn bulk_candidate(differing: &[ClauseReport]) -> Option<Vec<ClauseMatch>> {
    if differing.is_empty() {
        return None;
    }
    differing
        .iter()
        .map(|clause| match &clause.state {
            ClauseState::Differs { written, .. } => {
                let rules = same_words(&clause.text, written)?;
                Some(ClauseMatch {
                    face: clause.face,
                    printed: clause.text.clone(),
                    written: written.clone(),
                    rules: if rules.is_empty() {
                        vec![Rule::Punctuation]
                    } else {
                        rules
                    },
                })
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests;
