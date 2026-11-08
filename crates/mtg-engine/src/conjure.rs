//! Conjured cards: the `CardConjured` event makes a new card in a hand, a library, a graveyard or exile.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::error::EngineError;
use crate::fold::invalid;
use crate::object::GameObject;
use crate::state::GameState;
use crate::zones::{Placement, Zone};

/// A card that was conjured: a new object of its owner in a zone, put on top of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CardConjured {
    /// The card's object.
    pub object: ObjectId,
    /// Who owns it.
    pub owner: PlayerId,
    /// Where it was created: a hand, a library, a graveyard or exile.
    pub zone: Zone,
    /// What it is called, for the log.
    pub name: String,
    /// Its characteristics and abilities, so a log folds to the same game with no card data at hand.
    #[cfg_attr(feature = "ts", ts(skip))]
    pub definition: Arc<CardDefinition>,
}

/// Folds the event: the card is a new object in the zone, on top of it.
pub(crate) fn fold(state: &mut GameState, event: &CardConjured) -> Result<(), EngineError> {
    let id = event.object;
    if state.objects.contains_key(&id) || state.ceased.contains_key(&id) {
        return Err(invalid(format!("{id:?} already exists")));
    }
    if !state.is_alive(event.owner) {
        return Err(invalid(format!(
            "{:?} is out of the game and cannot own a conjured card",
            event.owner
        )));
    }
    if event.name != event.definition.name {
        return Err(invalid(
            "a conjured card is called what its definition is called",
        ));
    }
    let owned = match event.zone {
        Zone::Hand(player) | Zone::Library(player) | Zone::Graveyard(player) => {
            player == event.owner
        }
        Zone::Exile => true,
        _ => false,
    };
    if !owned {
        return Err(invalid(format!(
            "a card is not conjured into {:?}",
            event.zone
        )));
    }
    crate::copies::note_definition(state, &event.definition);
    let card = GameObject::new(id, event.owner, Arc::clone(&event.definition), event.zone);
    state.objects.insert(id, card);
    state.zones.insert(event.zone, id, Placement::Top);
    state.next_object = state.next_object.max(id.get() + 1);
    Ok(())
}
