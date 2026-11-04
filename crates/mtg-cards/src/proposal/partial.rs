//! The proof of a partial translation: every printed clause is marked as running automatically or left by hand, or the card is refused by name (`docs/cards-dsl.md`, Partial translations).

use serde::{Deserialize, Serialize};

/// One line of a Forge script (`K:`, `A:`, `T:`, `S:` or `R:`) and what the translator made of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerLine {
    /// The face of the card the line belongs to.
    pub face: usize,
    /// The line's tag letter.
    pub tag: String,
    /// The line's own words: its `SpellDescription$`, `TriggerDescription$` or `Description$`; for a keyword the body.
    pub text: String,
    /// The indices, on the proposal's face, of the abilities the line gave; empty for a gap.
    pub abilities: Vec<usize>,
    /// The construct that stopped the line; `None` when it translated.
    pub gap: Option<String>,
}
