//! Flashback (CR 702.34): "You may cast this card from your graveyard for its flashback cost. Then exile it."
//!
//! The cost is an alternative cost chosen with the cast from the graveyard (`ways.rs`); the replacement step exiles the
//! spell cast for it when it would leave the stack (`replace.rs`).

use super::KeywordRules;

/// The rules of flashback.
#[derive(Debug)]
pub(super) struct Flashback;

impl KeywordRules for Flashback {}
