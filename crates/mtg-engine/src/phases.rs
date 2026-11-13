//! Boss phases, a campaign rule: a seat set up with phases gets the next phase's emblem in its command zone, as a
//! state-based action, once its life total is at or below that phase's threshold. Phases begin in order, one per pass.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::error::EngineError;
use crate::events::Event;
use crate::fold::{invalid, reset_passes};
use crate::object::{GameObject, ObjectKind};
use crate::state::GameState;
use crate::zones::{Placement, Zone};

/// One phase of a boss seat: the emblem it gets when its life total becomes `at_life` or less.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BossPhase {
    /// The life total at or below which the phase begins.
    pub at_life: i32,
    /// The phase's emblem, made with [`emblem_definition`](crate::w6b_emblems::emblem_definition).
    pub emblem: Arc<CardDefinition>,
}

/// The phases that begin in this pass, their emblems numbered after the `taken` new objects of the pass's earlier
/// events.
pub(crate) fn begin(state: &GameState, taken: u64) -> Vec<Event> {
    let mut next = state.next_object().get() + taken;
    let mut events = Vec::new();
    for player in state
        .players()
        .iter()
        .filter(|p| p.is_in_game() && !p.is_spirit())
    {
        let Some(phase) = player.phases.get(usize::from(player.phase)) else {
            continue;
        };
        if player.life() > 0 && player.life() <= phase.at_life {
            events.push(Event::PhaseBegan {
                player: player.id(),
                emblem: ObjectId(next),
            });
            next += 1;
        }
    }
    events
}

/// Folds `PhaseBegan`: the player's next phase begins and its emblem is a new object in their command zone.
pub(crate) fn fold(
    state: &mut GameState,
    id: PlayerId,
    emblem: ObjectId,
) -> Result<(), EngineError> {
    if state.objects.contains_key(&emblem) || state.ceased.contains_key(&emblem) {
        return Err(invalid(format!("{emblem:?} already exists")));
    }
    let player = state
        .players
        .iter_mut()
        .find(|p| p.id() == id)
        .ok_or_else(|| invalid(format!("no player {id:?}")))?;
    let Some(phase) = player.phases.get(usize::from(player.phase)).cloned() else {
        return Err(invalid(format!("{id:?} has no phase left")));
    };
    player.phase += 1;
    let zone = Zone::Command(id);
    let mut object = GameObject::new(emblem, id, Arc::clone(&phase.emblem), zone);
    object.kind = ObjectKind::Emblem;
    crate::copies::note_definition(state, &phase.emblem);
    state.objects.insert(emblem, object);
    state.zones.insert(zone, emblem, Placement::Top);
    state.next_object = state.next_object.max(emblem.get() + 1);
    reset_passes(state);
    Ok(())
}
