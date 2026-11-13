//! Spirits, a campaign rule: a seat set up with a Spirit emblem does not lose at 0 life while a teammate lives. It
//! becomes a Spirit instead: its cards outside the library and the command zone leave the game, it gets the emblem
//! (its haunt abilities) and it loses no more on its own. A team whose players are all Spirits, or are losing, loses
//! together, so a Spirit never wins alone.
//!
//! A Spirit is out of the fight's reach: no opponent can attack it or target it. It starts with one ectoplasm
//! counter on the player, the resource its haunt abilities spend, and `Effect::Revive` returns it to life.

use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_cards::value::CounterKind;
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
            events.push(ectoplasm_change(*player, 1));
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

/// The objects a Spirit loses: its cards outside its library and the command zone leave the game; its haunt abilities
/// on the stack stay.
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
                && !matches!(o.zone(), Zone::Command(_) | Zone::Library(_) | Zone::Stack)
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

/// The name of the player counter a Spirit's haunt abilities spend.
pub const ECTOPLASM: &str = "ectoplasm";

/// The ectoplasm counter kind.
pub fn ectoplasm() -> CounterKind {
    CounterKind::Other(ECTOPLASM.to_owned())
}

/// The event that gives `player` `delta` ectoplasm (or takes it, when negative).
fn ectoplasm_change(player: PlayerId, delta: i32) -> Event {
    Event::CountersChanged(crate::counters::CountersChanged {
        on: crate::choose::Target::Player(player),
        counter: ectoplasm(),
        delta,
    })
}

/// Whether `player` may be attacked: a Spirit never is.
pub(crate) fn attackable(state: &GameState, player: PlayerId) -> bool {
    !state.player(player).is_some_and(|p| p.spirit)
}

/// Whether `player` is a Spirit that a spell or ability `controller` controls cannot target: an opponent's never can,
/// the Spirit's own and its teammates' can.
pub(crate) fn shielded(state: &GameState, player: PlayerId, controller: PlayerId) -> bool {
    state
        .player(player)
        .is_some_and(|p| p.spirit && p.is_in_game())
        && controller != player
        && !state.are_teammates(controller, player)
}

/// The events that return the Spirit `player` to life: its haunt emblems leave the game, it is a living player with
/// `life`, its ectoplasm gone, and draws `cards`. Nothing for a player who is not a Spirit in the game.
pub(crate) fn revive(state: &GameState, player: PlayerId, life: i32, cards: usize) -> Vec<Event> {
    let Some(seat) = state.player(player) else {
        return Vec::new();
    };
    if !seat.spirit || !seat.is_in_game() {
        return Vec::new();
    }
    let name = seat.spirit_form.as_ref().map(|form| form.name.clone());
    let mut events: Vec<Event> = state
        .zone(Zone::Command(player))
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.kind == ObjectKind::Emblem && Some(&o.card().name) == name.as_ref())
        .map(|o| Event::ObjectLeftGame { object: o.id() })
        .collect();
    let held = seat.counters_of(&ectoplasm());
    if held > 0 {
        events.push(ectoplasm_change(
            player,
            -i32::try_from(held).unwrap_or(i32::MAX),
        ));
    }
    events.push(Event::SpiritRevived { player, life });
    events.extend(crate::steps::draw(state, player, cards));
    events
}

/// Folds `SpiritRevived`: the player is no longer a Spirit and its life total is `life`.
pub(crate) fn fold_revived(
    state: &mut GameState,
    id: PlayerId,
    life: i32,
) -> Result<(), EngineError> {
    let player = state
        .players
        .iter_mut()
        .find(|p| p.id() == id)
        .ok_or_else(|| invalid(format!("no player {id:?}")))?;
    if !player.is_in_game() || !player.spirit {
        return Err(invalid(format!("{id:?} is not a Spirit")));
    }
    player.spirit = false;
    player.life = life;
    reset_passes(state);
    Ok(())
}
