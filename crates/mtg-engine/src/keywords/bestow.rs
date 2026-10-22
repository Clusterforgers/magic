//! Bestow (CR 702.103): "You may cast this card as an Aura spell with enchant creature for its bestow cost."
//!
//! The cost is an alternative cost chosen with the cast from a hand (`ways.rs`). While the spell and the permanent are
//! bestowed they are non-creature Auras that enchant creatures (`GameObject::bestowed`, `attach.rs`); the keyword itself
//! adds no ability.

use super::KeywordRules;

/// The rules of bestow.
#[derive(Debug)]
pub(super) struct Bestow;

impl KeywordRules for Bestow {}
