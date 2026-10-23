//! Mayhem (CR 702.190): "You may cast this card from your graveyard for [cost] if you discarded it this turn. Timing rules
//! still apply."
//!
//! The cost is an alternative cost chosen with the cast (`ways.rs`), open from the graveyard of the card's owner while the
//! turn's tally of discards (`GameState::discarded_this_turn`) holds this very card. The spell remembers it was cast that way
//! for "if its mayhem cost was paid" (`Condition::CastWith`).

use super::KeywordRules;

/// The rules of mayhem.
#[derive(Debug)]
pub(super) struct Mayhem;

impl KeywordRules for Mayhem {}
