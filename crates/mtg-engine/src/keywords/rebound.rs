//! Rebound (CR 702.88): "If you cast this spell from your hand, exile it as it resolves. At the beginning of your next
//! upkeep, you may cast this card from exile without paying its mana cost."
//!
//! The replacement step exiles the spell that resolved and sets up the delayed trigger that casts it (`replace.rs`).

use super::KeywordRules;

/// The rules of rebound.
#[derive(Debug)]
pub(super) struct Rebound;

impl KeywordRules for Rebound {}
