//! Retrace (CR 702.81): "You may cast this card from your graveyard by discarding a land card in addition to paying its
//! other costs."
//!
//! The cast from the graveyard is a way (`ways.rs`): the printed mana cost and the discard of a land card, picked with the
//! cast like any part of a spell's cost. The spell goes to the graveyard as it resolves, so it can be cast that way again.

use super::KeywordRules;

/// The rules of retrace.
#[derive(Debug)]
pub(super) struct Retrace;

impl KeywordRules for Retrace {}
