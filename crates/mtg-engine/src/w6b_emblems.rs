//! Emblems (CR 114): the `EmblemCreated` event and the object it makes in its owner's command zone, whose abilities
//! work from there for the rest of the game; it leaves the game only with its owner (CR 800.4a).

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::assemble::token_definition;
use mtg_cards::card::TokenSpec;
use mtg_cards::refs::ZoneKind;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::error::EngineError;
use crate::fold::{invalid, reset_passes};
use crate::object::{GameObject, ObjectKind};
use crate::state::GameState;
use crate::zones::{Placement, Zone};

/// The name every emblem goes by: an emblem has no characteristics but its abilities (CR 114.3).
pub const W6B_EMBLEM_NAME: &str = "Emblem";

/// A player got an emblem: a new object in their command zone that they own and control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EmblemCreated {
    /// The emblem's object.
    pub object: ObjectId,
    /// Who gets it.
    pub owner: PlayerId,
    /// Its abilities, so a log folds to the same game with no card data at hand.
    #[cfg_attr(feature = "ts", ts(skip))]
    pub definition: Arc<CardDefinition>,
}

/// The definition of an emblem with these abilities: each works from the command zone, and only the ones the engine
/// runs are kept (as for a token).
pub(crate) fn w6b_definition(abilities: &[Ability]) -> CardDefinition {
    let spec = TokenSpec {
        name: W6B_EMBLEM_NAME.to_owned(),
        colors: Vec::new(),
        supertypes: Vec::new(),
        types: Vec::new(),
        subtypes: Vec::new(),
        power: None,
        toughness: None,
        abilities: abilities.iter().map(w6b_from_command).collect(),
    };
    token_definition(&spec, &crate::caps::caps())
}

/// The ability as it works on an emblem: from the command zone.
fn w6b_from_command(ability: &Ability) -> Ability {
    let mut moved = ability.clone();
    match &mut moved {
        Ability::Activated { from, .. }
        | Ability::Triggered { from, .. }
        | Ability::Static { from, .. }
        | Ability::Rule { from, .. }
        | Ability::Replacement { from, .. } => *from = ZoneKind::Command,
        _ => {}
    }
    moved
}

/// Folds the event: the emblem is a new object in its owner's command zone.
pub(crate) fn fold(state: &mut GameState, event: &EmblemCreated) -> Result<(), EngineError> {
    let id = event.object;
    if state.objects.contains_key(&id) || state.ceased.contains_key(&id) {
        return Err(invalid(format!("{id:?} already exists")));
    }
    if !state.is_alive(event.owner) {
        return Err(invalid(format!(
            "{:?} is out of the game and cannot get an emblem",
            event.owner
        )));
    }
    let zone = Zone::Command(event.owner);
    let mut emblem = GameObject::new(id, event.owner, Arc::clone(&event.definition), zone);
    emblem.kind = ObjectKind::Emblem;
    crate::copies::note_definition(state, &event.definition);
    state.objects.insert(id, emblem);
    state.zones.insert(zone, id, Placement::Top);
    state.next_object = state.next_object.max(id.get() + 1);
    reset_passes(state);
    Ok(())
}
