//! Folding events that move objects between zones.

use std::collections::BTreeSet;

use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{invalid, player_mut, reset_passes};
use crate::combat::AttackTarget;
use crate::error::EngineError;
use crate::events::Event;
use crate::object::{GameObject, PermanentState};
use crate::priority::PriorityState;
use crate::state::GameState;
use crate::zones::{Placement, Zone};

/// Moves `id` to `to`, creating or dropping permanent state and leaving combat as needed.
pub(super) fn move_object(
    state: &mut GameState,
    id: ObjectId,
    to: Zone,
    placement: Placement,
) -> Result<(), EngineError> {
    if !state.zones.exists(to) {
        return Err(invalid(format!("zone {to:?} does not exist")));
    }
    let object = state
        .objects
        .get_mut(&id)
        .ok_or_else(|| invalid(format!("unknown object {id:?}")))?;
    let from = object.zone;
    if !state.zones.remove(from, id) {
        return Err(invalid(format!("{id:?} is not in {from:?}")));
    }
    state.zones.insert(to, id, placement);
    object.zone = to;
    if from == Zone::Battlefield {
        object.permanent = None;
        state.combat.remove_object(id);
    }
    if to == Zone::Battlefield {
        object.permanent = Some(PermanentState {
            tapped: false,
            damage: 0,
            summoning_sick: true,
        });
    } else if to != Zone::Stack {
        object.controller = object.owner;
    }
    Ok(())
}

fn expect_in(state: &GameState, id: ObjectId, zone: Zone) -> Result<(), EngineError> {
    if state.zones.get(zone).contains(&id) {
        Ok(())
    } else {
        Err(invalid(format!("{id:?} is not in {zone:?}")))
    }
}

fn set_controller(state: &mut GameState, id: ObjectId, player: PlayerId) {
    if let Some(object) = state.objects.get_mut(&id) {
        object.controller = player;
    }
}

pub(super) fn apply(state: &mut GameState, event: &Event) -> Result<(), EngineError> {
    match event {
        Event::LandPlayed { player, object } => {
            expect_in(state, *object, Zone::Hand(*player))?;
            move_object(state, *object, Zone::Battlefield, Placement::Top)?;
            set_controller(state, *object, *player);
            state.turn.lands_played += 1;
            reset_passes(state);
        }
        Event::SpellCast { player, object } => {
            expect_in(state, *object, Zone::Hand(*player))?;
            move_object(state, *object, Zone::Stack, Placement::Top)?;
            set_controller(state, *object, *player);
            reset_passes(state);
        }
        Event::SpellResolved { object } => resolve_spell(state, *object)?,
        Event::CardDrawn { player, object } => {
            if state.zones.get(Zone::Library(*player)).last() != Some(object) {
                return Err(invalid("drawn card is not on top of the library"));
            }
            move_object(state, *object, Zone::Hand(*player), Placement::Top)?;
        }
        Event::CardDiscarded { player, object } => {
            expect_in(state, *object, Zone::Hand(*player))?;
            let owner = owner_of(state, *object)?;
            move_object(state, *object, Zone::Graveyard(owner), Placement::Top)?;
        }
        Event::CreatureDied { object, .. } => {
            expect_in(state, *object, Zone::Battlefield)?;
            let owner = owner_of(state, *object)?;
            move_object(state, *object, Zone::Graveyard(owner), Placement::Top)?;
        }
        Event::ObjectLeftGame { object } => leave_game(state, *object)?,
        other => return Err(invalid(format!("not a zone event: {other:?}"))),
    }
    Ok(())
}

fn owner_of(state: &GameState, id: ObjectId) -> Result<PlayerId, EngineError> {
    state
        .object(id)
        .map(GameObject::owner)
        .ok_or_else(|| invalid(format!("unknown object {id:?}")))
}

fn resolve_spell(state: &mut GameState, id: ObjectId) -> Result<(), EngineError> {
    if state.zones.get(Zone::Stack).last() != Some(&id) {
        return Err(invalid(format!("{id:?} is not the top of the stack")));
    }
    let types = state
        .object(id)
        .map(GameObject::base_types)
        .ok_or_else(|| invalid(format!("unknown object {id:?}")))?;
    if types.contains(CardType::Instant) || types.contains(CardType::Sorcery) {
        let owner = owner_of(state, id)?;
        move_object(state, id, Zone::Graveyard(owner), Placement::Top)?;
    } else {
        move_object(state, id, Zone::Battlefield, Placement::Top)?;
    }
    let active = state.turn.active;
    if let Some(holder) = state.turn_order_from(active).first().copied() {
        state.priority = Some(PriorityState {
            holder,
            passed: BTreeSet::default(),
        });
    }
    Ok(())
}

fn leave_game(state: &mut GameState, id: ObjectId) -> Result<(), EngineError> {
    let object = state
        .objects
        .remove(&id)
        .ok_or_else(|| invalid(format!("unknown object {id:?}")))?;
    if !state.zones.remove(object.zone, id) {
        return Err(invalid(format!("{id:?} is not in {:?}", object.zone)));
    }
    state.combat.remove_object(id);
    state
        .combat
        .remove_attackers_targeting(AttackTarget::Planeswalker(id));
    player_mut(state, object.owner).map(|_| ())
}
