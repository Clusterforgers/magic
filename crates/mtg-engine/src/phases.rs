//! Boss phases, a campaign rule: a seat set up with phases gets the next phase's emblem in its command zone, as a
//! state-based action, once the phase's start is reached: a life threshold, or the end step of the last turn of a
//! round (an extra turn does not advance the round). Phases begin in order, one per pass; the emblem's "when this
//! phase begins" ability (`TriggerPattern::PhaseBegins`) then uses the stack once.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::error::EngineError;
use crate::events::Event;
use crate::fold::{invalid, reset_passes};
use crate::object::{GameObject, ObjectKind};
use crate::state::GameState;
use crate::zones::{Placement, Zone};

/// When a boss phase begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhaseStart {
    /// Once the seat's life total is this or less.
    AtLife(i32),
    /// At the end step of the last turn of this round (`Turn::round`), or of a later round's last turn.
    AtRound(u32),
}

impl PhaseStart {
    /// Whether the phase begins now for `player`.
    fn reached(self, state: &GameState, player: &crate::player::Player) -> bool {
        match self {
            Self::AtLife(at) => player.life() > 0 && player.life() <= at,
            Self::AtRound(round) => {
                state.turn().step() == crate::turn::Step::End
                    && state.turn().round() >= round
                    && state.ends_round()
            }
        }
    }
}

/// One phase of a boss seat: the emblem it gets once the phase begins.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BossPhase {
    /// When the phase begins.
    pub begins: PhaseStart,
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
        if phase.begins.reached(state, player) {
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
