//! State-based actions (CR 704), applied in passes until nothing changes.

use crate::characteristics::{Characteristics, characteristics};
use crate::events::{DeathCause, Event};
use crate::object::PermanentState;
use crate::player::{LossReason, Player};
use crate::state::GameState;
use crate::zones::Zone;

fn loss_reason(state: &GameState, player: &Player) -> Option<LossReason> {
    if player.life() <= 0 {
        Some(LossReason::ZeroLife)
    } else if player.drew_from_empty {
        Some(LossReason::EmptyLibrary)
    } else {
        state.format().loss_condition(state, player.id())
    }
}

/// One simultaneous pass over players and creatures; empty once the state is stable.
pub(crate) fn check(state: &GameState) -> Vec<Event> {
    let mut events: Vec<Event> = state
        .players()
        .iter()
        .filter(|p| p.is_in_game())
        .filter_map(|p| {
            loss_reason(state, p).map(|reason| Event::PlayerLost {
                player: p.id(),
                reason,
            })
        })
        .collect();
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id) else {
            continue;
        };
        if !state.is_alive(object.controller()) {
            continue;
        }
        let Some(chars) = characteristics(state, id).filter(Characteristics::is_creature) else {
            continue;
        };
        let toughness = chars.toughness.unwrap_or(0);
        let damage = object.permanent().map_or(0, PermanentState::damage);
        let cause = if toughness <= 0 {
            DeathCause::ZeroToughness
        } else if i64::from(damage) >= i64::from(toughness) {
            DeathCause::LethalDamage
        } else {
            continue;
        };
        events.push(Event::CreatureDied { object: id, cause });
    }
    events
}

/// Housekeeping that follows stable passes: remove eliminated players' objects, drop ended effects and delayed triggers.
pub(crate) fn housekeeping(state: &GameState) -> Vec<Event> {
    let departing: Vec<Event> = state
        .objects()
        .filter(|o| !state.is_alive(o.owner()))
        .map(|o| Event::ObjectLeftGame { object: o.id() })
        .collect();
    if !departing.is_empty() {
        return departing;
    }
    let mut expired: Vec<Event> = state
        .effects()
        .iter()
        .filter(|e| !e.is_active(state))
        .map(|e| Event::EffectExpired { id: e.id })
        .collect();
    expired.extend(
        state
            .delayed()
            .iter()
            .filter(|d| d.lapsed(state))
            .map(|d| Event::DelayedTriggerExpired { id: d.id() }),
    );
    expired
}
