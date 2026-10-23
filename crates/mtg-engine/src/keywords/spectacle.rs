//! Spectacle (CR 702.137): "You may cast this spell by paying [cost] rather than its mana cost if an opponent lost life
//! this turn."
//!
//! The cost is an alternative cost chosen with the cast from a hand (`ways.rs`), open while the turn's record of life lost
//! (`GameState::life_lost_by_opponent_of`) holds a player the caster is against. The spell remembers it was cast that way
//! for "if its spectacle cost was paid" (`Condition::CastWith`).

use super::KeywordRules;

/// The rules of spectacle.
#[derive(Debug)]
pub(super) struct Spectacle;

impl KeywordRules for Spectacle {}
