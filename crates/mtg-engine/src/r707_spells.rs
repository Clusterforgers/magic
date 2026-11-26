//! Spells kept in exile and put back onto the stack as copies of themselves (`Effect::R707ExileSpell`,
//! `Effect::R707PutBack`, CR 707.10), and cards turned face up in exile (`Effect::R406TurnUpExiled`, CR 406.3).

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::choose::Choices;
use crate::state::GameState;
use crate::zones::Zone;

/// A card exiled from the stack and the spell it was: what it announced and who controlled it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct R707Kept {
    /// The card, as it is in exile.
    pub object: ObjectId,
    /// What the spell announced: modes, targets, X, optional costs and the way it was cast.
    #[cfg_attr(feature = "ts", ts(skip))]
    pub choices: Choices,
    /// Who controlled the spell.
    pub controller: PlayerId,
}

/// The card in exile keeps the spell; a card that has left exile keeps nothing.
pub(crate) fn fold_kept(state: &mut GameState, kept: &R707Kept) {
    if let Some(card) = state.objects.get_mut(&kept.object)
        && card.zone == Zone::Exile
    {
        card.r707_kept = Some(Box::new((kept.choices.clone(), kept.controller)));
    }
}

/// The card lying face down in exile lies face up from now on (CR 406.3).
pub(crate) fn r406_fold_turned_up(state: &mut GameState, object: ObjectId) {
    if let Some(card) = state.objects.get_mut(&object)
        && card.zone == Zone::Exile
    {
        card.face_down = None;
    }
}

/// Whether the card in exile keeps a spell to be put back.
pub(crate) fn keeps_spell(state: &GameState, object: ObjectId) -> bool {
    state
        .object(object)
        .is_some_and(|card| card.zone() == Zone::Exile && card.r707_kept.is_some())
}
