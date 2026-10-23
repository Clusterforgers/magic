//! Sagas (CR 714): the lore counter a Saga enters with, the one it gets as each precombat main phase begins and the
//! sacrifice of a Saga whose last chapter ability has left the stack. The chapter abilities are triggered abilities of
//! the card (`TriggerPattern::Chapter`) that the trigger detection fires as lore counters reach their number.

use mtg_cards::ability::Ability;
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::CounterKind;
use mtg_core::card::Subtype;
use mtg_core::ids::ObjectId;

use crate::ability_reader::abilities_of;
use crate::characteristics::seen_as;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::{Event, ZoneCause};
use crate::object::{GameObject, ObjectRef};
use crate::state::GameState;
use crate::zones::Zone;

/// The Saga subtype.
fn saga() -> Subtype {
    Subtype("Saga".to_owned())
}

/// One lore counter on `object`, for the object the card is (`counter`) once it has entered.
fn lore(on: ObjectRef) -> Event {
    Event::CountersChanged(CountersChanged {
        on: Target::Object(on),
        counter: CounterKind::lore(),
        delta: 1,
    })
}

/// The change of counters that gives the Saga card `id` its first lore counter as it enters the battlefield
/// (CR 714.3a), named for the new object the card becomes (CR 400.7).
///
/// It is proposed with the events that move the card, like any other change of counters, so a rule that changes what
/// enters with counters can reach it. `None` for a card that is not a Saga and one that is on the battlefield already.
pub(crate) fn entering(state: &GameState, id: ObjectId) -> Option<Event> {
    let object = state.object(id)?;
    if object.zone() == Zone::Battlefield || !object.card().subtypes.contains(&saga()) {
        return None;
    }
    Some(lore(ObjectRef {
        id,
        counter: object.counter() + 1,
    }))
}

/// The lore counters the Sagas of the active player get as their precombat main phase begins (CR 714.3b): a
/// turn-based action, so the chapter abilities they reach trigger once it is done.
pub(crate) fn precombat_counters(state: &GameState) -> Vec<Event> {
    let Some(active) = state.active_player() else {
        return Vec::new();
    };
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.controller() == active && is_saga(state, object))
        .map(|object| lore(object.reference()))
        .collect()
}

/// Whether the permanent is a Saga now: it prints the subtype or an effect gave it.
fn is_saga(state: &GameState, object: &GameObject) -> bool {
    seen_as(state, object.id())
        .is_some_and(|(chars, _)| chars.has_subtype(&object.card().subtypes, &saga()))
}

/// The number of the last chapter ability the permanent has; `None` for one with no chapter abilities.
pub(crate) fn final_chapter(state: &GameState, id: ObjectId) -> Option<u32> {
    abilities_of(state, id)
        .iter()
        .filter_map(|(_, rules)| chapter_of(rules))
        .max()
}

/// The number of the chapter an ability is.
fn chapter_of(rules: &Ability) -> Option<u32> {
    match rules {
        Ability::Triggered {
            when: TriggerPattern::Chapter { number },
            ..
        } => Some(*number),
        _ => None,
    }
}

/// Whether a chapter ability of `source` has triggered and not left the stack: it waits to be put there or is there.
fn has_chapter_waiting(state: &GameState, source: ObjectRef) -> bool {
    let waiting = state
        .pending_triggers()
        .iter()
        .any(|pending| pending.trigger().source == source && chapter_of(pending.rules()).is_some());
    waiting
        || state.stack().iter().any(|id| {
            state
                .object(*id)
                .and_then(GameObject::ability)
                .is_some_and(|put| put.source() == source && chapter_of(put.rules()).is_some())
        })
}

/// The Sagas that are sacrificed now (CR 714.4): those with as many lore counters as their last chapter number, or
/// more, that are not the source of a chapter ability on its way to the stack or on it. A Saga in `gone` leaves in
/// this pass already.
pub(crate) fn sacrifices(state: &GameState, gone: &[ObjectId]) -> Vec<Event> {
    let lore = CounterKind::lore();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| !gone.contains(&object.id()) && state.is_alive(object.controller()))
        .filter(|object| {
            let held = object
                .permanent()
                .map_or(0, |permanent| permanent.counters_of(&lore));
            held > 0
                && final_chapter(state, object.id()).is_some_and(|last| held >= last)
                && !has_chapter_waiting(state, object.reference())
        })
        .map(|object| Event::ZoneChanged {
            object: object.id(),
            from: Zone::Battlefield,
            to: Zone::Graveyard(object.owner()),
            cause: ZoneCause::Sacrificed,
        })
        .collect()
}
