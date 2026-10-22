//! Jump-start (CR 702.133): "You may cast this card from your graveyard by discarding a card in addition to paying its other
//! costs. Then exile this card."
//!
//! The cast from the graveyard is a way (`ways.rs`): the printed mana cost and the discard of a card, picked with the cast
//! like any part of a spell's cost. The replacement step exiles the spell cast that way when it would leave the stack
//! (`replace.rs`), as it does for flashback.

use super::KeywordRules;

/// The rules of jump-start.
#[derive(Debug)]
pub(super) struct JumpStart;

impl KeywordRules for JumpStart {}
