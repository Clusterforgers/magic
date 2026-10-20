//! Folding events that move objects between zones.

use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use super::opening::same_cards;
use super::{invalid, player_mut, priority_to_active, reset_passes};
use crate::choose::Choices;
use crate::combat::AttackTarget;
use crate::error::EngineError;
use crate::events::Event;
use crate::library::Scried;
use crate::object::{GameObject, ObjectKind, PermanentState};
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
    // A new zone makes it a new object (CR 400.7); what was announced for it on the stack goes with the stack.
    object.counter += 1;
    if from == Zone::Stack {
        object.choices = Choices::default();
    }
    if from == Zone::Battlefield {
        object.permanent = None;
        state.combat.remove_object(id);
        // Creatures attacking a planeswalker that is gone are out of combat, as when its owner leaves the game.
        state
            .combat
            .remove_attackers_targeting(AttackTarget::Planeswalker(id));
    }
    if to == Zone::Battlefield {
        object.permanent = Some(PermanentState {
            tapped: false,
            damage: 0,
            deathtouched: false,
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
        Event::SpellCast {
            player,
            object,
            choices,
        } => {
            expect_in(state, *object, Zone::Hand(*player))?;
            move_object(state, *object, Zone::Stack, Placement::Top)?;
            set_controller(state, *object, *player);
            if let Some(spell) = state.objects.get_mut(object) {
                spell.choices.clone_from(choices);
            }
            reset_passes(state);
        }
        Event::SpellResolved { object } => resolve_spell(state, *object)?,
        Event::ZoneChanged {
            object, from, to, ..
        } => {
            expect_in(state, *object, *from)?;
            if state.object(*object).map(GameObject::kind) == Some(ObjectKind::Ability) {
                return Err(invalid(format!(
                    "{object:?} is an ability, which does not change zones"
                )));
            }
            move_object(state, *object, *to, Placement::Top)?;
        }
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

/// Puts the cards a scry looked at back: they are the top cards of the library, `top` goes on top and `bottom`
/// under everything else, each list as it lies from the top down.
pub(super) fn scry(state: &mut GameState, scried: &Scried) -> Result<(), EngineError> {
    let zone = Zone::Library(scried.player);
    let library = state.zones.get(zone);
    let looked: Vec<ObjectId> = scried.top.iter().chain(&scried.bottom).copied().collect();
    let Some(split) = library.len().checked_sub(looked.len()) else {
        return Err(invalid(
            "a scry looked at more cards than the library holds",
        ));
    };
    let (rest, on_top) = library.split_at(split);
    if !same_cards(on_top, &looked) {
        return Err(invalid(
            "a scry did not put back the top cards of the library",
        ));
    }
    let arranged = scried
        .bottom
        .iter()
        .rev()
        .chain(rest)
        .chain(scried.top.iter().rev())
        .copied()
        .collect();
    state.zones.set(zone, arranged);
    Ok(())
}

/// Moves cards that are in a library to its bottom, in the order given from the top down.
pub(super) fn bottom(
    state: &mut GameState,
    player: PlayerId,
    cards: &[ObjectId],
) -> Result<(), EngineError> {
    let zone = Zone::Library(player);
    let library = state.zones.get(zone);
    let distinct = cards
        .iter()
        .enumerate()
        .all(|(at, card)| !cards[..at].contains(card));
    if !distinct || !cards.iter().all(|card| library.contains(card)) {
        return Err(invalid(
            "bottomed cards are not all in the library, once each",
        ));
    }
    let arranged = cards
        .iter()
        .rev()
        .chain(library.iter().filter(|card| !cards.contains(card)))
        .copied()
        .collect();
    state.zones.set(zone, arranged);
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
    priority_to_active(state);
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
    if state.resolving.as_ref().is_some_and(|r| r.object == id) {
        state.resolving = None;
    }
    state.combat.remove_object(id);
    state
        .combat
        .remove_attackers_targeting(AttackTarget::Planeswalker(id));
    player_mut(state, object.owner).map(|_| ())
}
