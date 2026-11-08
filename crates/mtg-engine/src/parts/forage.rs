//! Foraging as a cost: the payer exiles three cards from their graveyard or sacrifices a Food they control. The pick is
//! either one Food or three cards of the graveyard, nothing else.

use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::Subtype;
use mtg_core::ids::ObjectId;

use super::Payer;
use crate::events::Event;
use crate::state::GameState;
use crate::zones::Zone;

/// The cards a forage exiles.
pub(super) const CARDS: usize = 3;

fn food() -> Selector {
    Selector(vec![Pred::Subtype(Subtype("Food".to_owned()))])
}

/// The Foods the payer controls.
pub(super) fn foods(payer: &Payer<'_>) -> Vec<ObjectId> {
    super::sacrifice::candidates(payer, &food())
}

/// The cards in the payer's graveyard.
pub(super) fn cards(payer: &Payer<'_>) -> Vec<ObjectId> {
    super::exile::candidates(payer, &Selector::default())
}

/// The Foods, and the graveyard when it holds enough cards to be exiled: what the player may choose between.
pub(super) fn candidates(payer: &Payer<'_>) -> Vec<ObjectId> {
    let mut found = foods(payer);
    let graveyard = cards(payer);
    if graveyard.len() >= CARDS {
        found.extend(graveyard);
    }
    found
}

fn in_graveyard(state: &GameState, id: ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|object| state.zone(Zone::Graveyard(object.owner())).contains(&id))
}

/// Whether `picked` is one Food, or three cards of the graveyard.
pub(super) fn valid(state: &GameState, picked: &[ObjectId]) -> bool {
    match picked {
        [one] => !in_graveyard(state, *one),
        many => many.len() == CARDS && many.iter().all(|id| in_graveyard(state, *id)),
    }
}

/// Whether the player has nothing to choose between: a single Food and no graveyard to exile, or the three cards.
pub(super) fn alone(state: &GameState, candidates: &[ObjectId]) -> bool {
    let cards = candidates
        .iter()
        .filter(|id| in_graveyard(state, **id))
        .count();
    let foods = candidates.len() - cards;
    (foods == 1 && cards == 0) || (foods == 0 && cards == CARDS)
}

/// What the question asks.
pub(super) fn text() -> String {
    "Forage: sacrifice a Food or exile three cards from your graveyard.".to_owned()
}

/// A card in the graveyard is exiled, a Food is sacrificed.
pub(super) fn event(state: &GameState, object: ObjectId) -> Option<Event> {
    if in_graveyard(state, object) {
        super::exile::event(state, object)
    } else {
        super::sacrifice::event(state, object)
    }
}
