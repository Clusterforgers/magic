//! Prowl (CR 702.76): "You may cast this card by paying [cost] rather than its mana cost if a player was dealt combat damage
//! this turn by a source that was under your control and had any of this card's creature types."
//!
//! The cost is an alternative cost chosen with the cast (`ways.rs`), open from a hand while the turn's record of combat damage
//! (`GameState::dealt_combat_damage_with`) holds a creature of the caster's that shared a creature type with the card. The
//! spell remembers it was cast that way for "if its prowl cost was paid" (`Condition::CastWith`).

use super::KeywordRules;

/// The rules of prowl.
#[derive(Debug)]
pub(super) struct Prowl;

impl KeywordRules for Prowl {}
