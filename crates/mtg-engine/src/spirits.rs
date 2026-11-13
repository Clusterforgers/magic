//! Spirits, a campaign rule: a seat set up with a Spirit emblem does not lose at 0 life while a teammate lives. It
//! becomes a Spirit instead: its cards outside the library and the command zone leave the game, it gets the emblem
//! (its haunt abilities) and it loses no more on its own. A team whose players are all Spirits, or are losing, loses
//! together, so a Spirit never wins alone.

use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_core::ids::{ObjectId, PlayerId};

use crate::error::EngineError;
use crate::events::Event;
use crate::fold::{invalid, reset_passes};
use crate::object::{GameObject, ObjectKind};
use crate::player::LossReason;
use crate::state::GameState;
use crate::zones::{Placement, Zone};

/// The losses of a state-based pass with the Spirit rule applied: a seat with a Spirit emblem that would lose for 0
/// life becomes a Spirit while a teammate stays alive; a team left with no living player loses with its Spirits.
pub(crate) fn settle(state: &GameState, losses: Vec<Event>) -> Vec<Event> {
    if state.players().iter().all(|p| p.spirit_form.is_none()) {
        return losses;
    }
    let losing: BTreeSet<PlayerId> = losses
        .iter()
        .filter_map(|event| match event {
            Event::PlayerLost { player, .. } => Some(*player),
            _ => None,
        })
        .collect();
    let can_turn = |id: PlayerId, reason: &LossReason| {
        *reason == LossReason::ZeroLife
            && state
                .player(id)
                .is_some_and(|p| p.spirit_form.is_some() && !p.spirit)
    };
    let living = |id: PlayerId| {
        state
            .player(id)
            .is_some_and(|p| p.is_in_game() && !p.spirit && !losing.contains(&id))
    };
    let mut next = state.next_object().get();
    let mut events = Vec::new();
    let mut spirits = BTreeSet::new();
    for event in losses {
        if let Event::PlayerLost { player, reason } = &event
            && can_turn(*player, reason)
            && state.players().iter().any(|mate| {
                mate.id() != *player && state.are_teammates(mate.id(), *player) && living(mate.id())
            })
        {
            spirits.insert(*player);
            events.push(Event::BecameSpirit {
                player: *player,
                emblem: ObjectId(next),
            });
            next += 1;
            continue;
        }
        events.push(event);
    }
    // A Spirit whose team has no living player left loses with it.
    for spirit in state
        .players()
        .iter()
        .filter(|p| p.is_in_game() && p.spirit)
    {
        let id = spirit.id();
        if losing.contains(&id) {
            continue;
        }
        let team_alive = state
            .players()
            .iter()
            .any(|mate| state.are_teammates(mate.id(), id) && mate.id() != id && living(mate.id()));
        if !team_alive {
            events.push(Event::PlayerLost {
                player: id,
                reason: LossReason::ZeroLife,
            });
        }
    }
    events
}

/// The objects a Spirit loses: its cards outside its library and the command zone leave the game.
pub(crate) fn fading(state: &GameState) -> Vec<Event> {
    if state.players().iter().all(|p| !p.spirit) {
        return Vec::new();
    }
    state
        .objects()
        .filter(|o| {
            state
                .player(o.owner())
                .is_some_and(|p| p.spirit && p.is_in_game())
                && !matches!(o.zone(), Zone::Command(_) | Zone::Library(_))
        })
        .map(|o| Event::ObjectLeftGame { object: o.id() })
        .collect()
}

/// Folds `BecameSpirit`: the player is a Spirit and its Spirit emblem is a new object in its command zone.
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
    if !player.is_in_game() || player.spirit {
        return Err(invalid(format!("{id:?} cannot become a Spirit")));
    }
    let Some(definition) = player.spirit_form.clone() else {
        return Err(invalid(format!("{id:?} has no Spirit form")));
    };
    player.spirit = true;
    player.failed_draws = 0;
    let zone = Zone::Command(id);
    let mut object = GameObject::new(emblem, id, Arc::clone(&definition), zone);
    object.kind = ObjectKind::Emblem;
    crate::copies::note_definition(state, &definition);
    state.objects.insert(emblem, object);
    state.zones.insert(zone, emblem, Placement::Top);
    state.next_object = state.next_object.max(emblem.get() + 1);
    reset_passes(state);
    Ok(())
}
