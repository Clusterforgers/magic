//! Escape (CR 702.138): "You may cast this card from your graveyard by paying [cost] rather than its mana cost."
//!
//! The cost is an alternative cost chosen with the cast from the graveyard (`ways.rs`); its mana is paid and the other
//! cards it exiles are picked with the cast like any part of a spell's cost. The spell remembers the way
//! (`Condition::CastWith(CastWay::Escape)`), so a permanent knows that it escaped; nothing exiles the spell afterwards.

use super::KeywordRules;

/// The rules of escape.
#[derive(Debug)]
pub(super) struct Escape;

impl KeywordRules for Escape {}
