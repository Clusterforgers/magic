//! Battles (CR 310): the defense a battle enters with, the player who protects it, who may attack it and the
//! state-based action that puts a battle with no defense into its owner's graveyard.
//!
//! Defense is the defense counters of a permanent. A Siege asks its controller for an opponent as it enters
//! (`EntersChoice::Opponent`), who protects it; the creatures that attack it are blocked by that player and only
//! players the protector is an opponent of attack it (`combat::defenders_for`). Damage dealt to a battle takes that
//! many defense counters off it (the fold of `DamageDealt`). The Siege's own rules (the choice and the cast of the
//! card transformed once it is defeated) are the abilities of its card.

use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::{Event, ZoneCause};
use crate::object::{Chosen, GameObject, ObjectRef};
use crate::state::GameState;
use crate::zones::Zone;

/// Whether the object `id` is a battle on the battlefield, by the card type it prints.
pub(crate) fn is_battle(state: &GameState, id: ObjectId) -> bool {
    state.object(id).is_some_and(|object| {
        object.zone() == Zone::Battlefield && object.base_types().contains(CardType::Battle)
    })
}

/// The change of counters that gives the battle card `id` its printed defense as it enters the battlefield (CR 310.4b),
/// named for the new object the card becomes (CR 400.7).
///
/// It is proposed with the events that move the card, like any other change of counters, so a rule that changes what
/// enters with counters can reach it. `None` for a card that is not a battle, one that is on the battlefield already
/// and one with no printed defense.
pub(crate) fn entering(state: &GameState, id: ObjectId) -> Option<Event> {
    let object = state.object(id)?;
    if object.zone() == Zone::Battlefield || !object.base_types().contains(CardType::Battle) {
        return None;
    }
    let defense = object.card().defense.filter(|defense| *defense > 0)?;
    Some(Event::CountersChanged(CountersChanged {
        on: Target::Object(ObjectRef {
            id,
            counter: object.counter() + 1,
        }),
        counter: CounterKind::defense(),
        delta: i32::try_from(defense).ok()?,
    }))
}

/// The player who protects the battle `id`: the player its controller chose as it entered (CR 310.8), if any.
pub(crate) fn protector(state: &GameState, id: ObjectId) -> Option<PlayerId> {
    match state.object(id)?.permanent()?.chosen()? {
        Chosen::Player(player) => Some(*player),
        _ => None,
    }
}

/// Whether an ability of `source` is on its way to the stack or on it.
fn has_ability_waiting(state: &GameState, source: ObjectRef) -> bool {
    state
        .pending_triggers()
        .iter()
        .any(|pending| pending.trigger().source == source)
        || state.stack().iter().any(|id| {
            state
                .object(*id)
                .and_then(GameObject::ability)
                .is_some_and(|put| put.source() == source)
        })
}

/// The battles put into their owners' graveyards now (CR 704.5v): those with no defense that are not the source of an
/// ability that has triggered and not yet left the stack. A battle in `gone` leaves in this pass already.
pub(crate) fn state_based(state: &GameState, gone: &[ObjectId]) -> Vec<Event> {
    let defense = CounterKind::defense();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| !gone.contains(&object.id()) && state.is_alive(object.controller()))
        .filter(|object| {
            object.base_types().contains(CardType::Battle)
                && object
                    .permanent()
                    .is_some_and(|permanent| permanent.counters_of(&defense) == 0)
                && !has_ability_waiting(state, object.reference())
        })
        .map(|object| Event::ZoneChanged {
            object: object.id(),
            from: Zone::Battlefield,
            to: Zone::Graveyard(object.owner()),
            cause: ZoneCause::StateBased,
        })
        .collect()
}
