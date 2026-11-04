//! Source memory: the objects a source remembers by slot (`Effect::Remember`, `Ref::Remembered`, `Pred::ExiledWith`).

use std::collections::BTreeMap;

use mtg_cards::refs::EXILED_SLOT;
use serde::{Deserialize, Serialize};

use crate::events::{Event, ZoneCause};
use crate::object::ObjectRef;
use crate::state::GameState;
use crate::zones::Zone;

/// What each source remembers: the objects by slot, each as the object it was when remembered (CR 400.7).
pub type Memory = BTreeMap<(ObjectRef, String), Vec<ObjectRef>>;

/// A source remembered objects, or forgot a slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Remembered {
    /// The object whose memory changes.
    pub source: ObjectRef,
    /// The name of the memory.
    pub slot: String,
    /// The objects added to the slot; the slot is emptied first when `replace` is set.
    pub objects: Vec<ObjectRef>,
    /// The slot starts empty before the objects are added.
    #[serde(default)]
    pub replace: bool,
}

/// Applies the change to the memory of the game.
pub(crate) fn fold(state: &mut GameState, change: &Remembered) {
    let key = (change.source, change.slot.clone());
    if change.replace {
        state.memory.remove(&key);
    }
    let held = state.memory.entry(key.clone()).or_default();
    for object in &change.objects {
        if !held.contains(object) {
            held.push(*object);
        }
    }
    if held.is_empty() {
        state.memory.remove(&key);
    }
}

/// An object that changes zones is a new object: its place in any memory is gone, and what it remembered goes with it
/// unless an ability of it can still read the memory: its leaves-the-battlefield trigger (Worldgorger Dragon returns the
/// permanents it exiled) or a delayed trigger. That memory stays until the last such ability has ended, or, for a source
/// no ability waits on, until the next step begins.
pub(crate) fn forget_object(state: &mut GameState, event: &Event) {
    if state.memory.is_empty() {
        return;
    }
    match event {
        Event::ZoneChanged { object, .. } => {
            let id = *object;
            for held in state.memory.values_mut() {
                held.retain(|named| named.id != id);
            }
            let forgotten = state.object(id).and_then(|left| {
                let me = left.reference();
                (!has_trigger(left) && !waiting(state, me, Waiting::default())).then_some(me)
            });
            if let Some(me) = forgotten {
                state.memory.retain(|(source, _), _| *source != me);
            }
        }
        Event::ResolutionEnded { object, .. } => {
            let source = state
                .object(*object)
                .and_then(|item| item.ability())
                .map(crate::abilities::AbilityObject::source);
            if let Some(source) = source {
                let ending = Waiting {
                    stacked: Some(*object),
                    queued: 0,
                };
                forget_if_unwaited(state, source, ending);
            }
        }
        Event::TriggerRemoved { source, .. } => {
            let ending = Waiting {
                stacked: None,
                queued: 1,
            };
            forget_if_unwaited(state, *source, ending);
        }
        Event::StepBegan { .. } => {
            let sources: Vec<ObjectRef> = state.memory.keys().map(|(source, _)| *source).collect();
            for source in sources {
                forget_if_unwaited(state, source, Waiting::default());
            }
        }
        _ => return,
    }
    state.memory.retain(|_, held| !held.is_empty());
}

/// The abilities that do not count as waiting because they are the one that is ending.
#[derive(Debug, Clone, Copy, Default)]
struct Waiting {
    /// The ability on the stack that is ending.
    stacked: Option<mtg_core::ids::ObjectId>,
    /// How many of the queued triggers of the source are ending.
    queued: usize,
}

/// Whether the printed rules of the object hold a triggered ability, which may read what it remembered once it has left.
fn has_trigger(object: &crate::object::GameObject) -> bool {
    object
        .card()
        .rules
        .iter()
        .any(|rule| matches!(**rule, mtg_cards::ability::Ability::Triggered { .. }))
}

/// Whether an ability of `source` is on the stack, waits to go there, or is a delayed trigger, the ending ones apart.
fn waiting(state: &GameState, source: ObjectRef, ending: Waiting) -> bool {
    let on_stack = state.stack().iter().any(|id| {
        Some(*id) != ending.stacked
            && state
                .object(*id)
                .and_then(|item| item.ability())
                .is_some_and(|ability| ability.source() == source)
    });
    let queued = state
        .pending_triggers()
        .iter()
        .filter(|pending| pending.trigger().source == source)
        .count();
    on_stack || queued > ending.queued || state.delayed().iter().any(|delayed| delayed.source == source)
}

/// Drops the memory of a source that has left once no ability waits on it.
fn forget_if_unwaited(state: &mut GameState, source: ObjectRef, ending: Waiting) {
    let left = state
        .object(source.id)
        .is_none_or(|object| object.counter() != source.counter);
    if left && !waiting(state, source, ending) {
        state.memory.retain(|(held, _), _| *held != source);
    }
}

/// The event that makes a permanent source remember what its effect exiled in the `exiled` slot, so `Pred::ExiledWith`
/// finds it whatever the effect (CR 607.2a); the objects are those the exile events make, as they are in exile.
pub(crate) fn exiled_by(source: ObjectRef, events: &[Event], state: &GameState) -> Option<Event> {
    let live = state.object(source.id).is_some_and(|object| {
        object.counter() == source.counter && object.zone() == Zone::Battlefield
    });
    if !live {
        return None;
    }
    let objects: Vec<ObjectRef> = events
        .iter()
        .filter_map(|event| match event {
            Event::ZoneChanged {
                object,
                to: Zone::Exile,
                cause: ZoneCause::Exiled | ZoneCause::ExiledUntil { .. },
                ..
            } if *object != source.id => state.object(*object).map(|was| ObjectRef {
                id: *object,
                counter: was.counter() + 1,
            }),
            _ => None,
        })
        .collect();
    (!objects.is_empty()).then(|| {
        Event::Remembered(Box::new(Remembered {
            source,
            slot: EXILED_SLOT.to_owned(),
            objects,
            replace: false,
        }))
    })
}

/// The objects the source remembers in the slot that are still the objects remembered.
pub(crate) fn recall(state: &GameState, source: ObjectRef, slot: &str) -> Vec<ObjectRef> {
    state
        .memory
        .get(&(source, slot.to_owned()))
        .into_iter()
        .flatten()
        .copied()
        .filter(|named| {
            state
                .object(named.id)
                .is_some_and(|object| object.counter() == named.counter)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: a source remembers objects by slot through events, in the order remembered and without repeats; what a source remembers ends when the source or a remembered object changes zones; a permanent remembers what its effects exile, and the memory is part of the hashed state so a replay of the events rebuilds it.
    use mtg_cards::refs::Ref;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::hash::stable_hash;
    use crate::resolve::Scope;

    const ME: PlayerId = PlayerId(0);

    /// Seat 0 controls Source, A and B on the battlefield and holds H.
    fn game() -> GameState {
        state(
            vec![vec![creature("H", 1, 1)], vec![]],
            vec![
                vec![
                    creature("Source", 1, 1),
                    creature("A", 1, 1),
                    creature("B", 1, 1),
                ],
                vec![],
            ],
        )
    }

    fn named(game: &GameState, name: &str) -> ObjectRef {
        game.objects()
            .find(|object| object.card().name == name)
            .map(crate::object::GameObject::reference)
            .unwrap()
    }

    fn remember(game: &mut GameState, slot: &str, names: &[&str], replace: bool) {
        let objects = names.iter().map(|name| named(game, name)).collect();
        let change = Remembered {
            source: named(game, "Source"),
            slot: slot.to_owned(),
            objects,
            replace,
        };
        apply_event(game, &Event::Remembered(Box::new(change))).unwrap();
    }

    fn recalled(game: &GameState, slot: &str) -> Vec<String> {
        recall(game, named(game, "Source"), slot)
            .into_iter()
            .map(|object| game.object(object.id).unwrap().card().name.clone())
            .collect()
    }

    fn leave(game: &mut GameState, name: &str) {
        let object = named(game, name);
        let from = game.object(object.id).unwrap().zone();
        let event = Event::ZoneChanged {
            object: object.id,
            from,
            to: Zone::Exile,
            cause: ZoneCause::Moved,
        };
        apply_event(game, &event).unwrap();
    }

    #[rstest]
    #[case::one(&[("A", false)], &["A"])]
    #[case::two_in_order(&[("B,A", false)], &["B", "A"])]
    #[case::added_later(&[("A", false), ("B", false)], &["A", "B"])]
    #[case::no_repeat(&[("A", false), ("A,B", false)], &["A", "B"])]
    #[case::replaced(&[("A", false), ("B", true)], &["B"])]
    #[case::forgotten(&[("A", false), ("", true)], &[])]
    fn remembering_adds_in_order_without_repeats(
        #[case] steps: &[(&str, bool)],
        #[case] expected: &[&str],
    ) {
        let mut game = game();
        for (names, replace) in steps {
            let names: Vec<&str> = names.split(',').filter(|name| !name.is_empty()).collect();
            remember(&mut game, "slot", &names, *replace);
        }
        assert_eq!(recalled(&game, "slot"), expected);
        assert_eq!(recalled(&game, "other"), Vec::<String>::new());
    }

    #[rstest]
    #[case::the_source_leaves("Source", &[])]
    #[case::a_remembered_object_leaves("A", &["B"])]
    #[case::an_unrelated_object_leaves("H", &["A", "B"])]
    fn a_zone_change_ends_the_memory_of_the_object(#[case] leaving: &str, #[case] kept: &[&str]) {
        let mut game = game();
        remember(&mut game, "slot", &["A", "B"], false);
        let source = named(&game, "Source");
        leave(&mut game, leaving);
        let left = game.memory.values().map(Vec::len).sum::<usize>();
        assert_eq!(left, kept.len());
        let names: Vec<_> = recall(&game, source, "slot")
            .into_iter()
            .map(|object| game.object(object.id).unwrap().card().name.clone())
            .collect();
        assert_eq!(names, kept);
    }

    #[test]
    fn an_object_that_came_back_is_not_the_one_remembered() {
        let mut game = game();
        remember(&mut game, "slot", &["A"], false);
        leave(&mut game, "A");
        let back = named(&game, "A");
        let event = Event::ZoneChanged {
            object: back.id,
            from: Zone::Exile,
            to: Zone::Battlefield,
            cause: ZoneCause::Moved,
        };
        apply_event(&mut game, &event).unwrap();
        assert_eq!(recalled(&game, "slot"), Vec::<String>::new());
    }

    #[rstest]
    #[case::a_permanent("Source", &["A", "B"], 1)]
    #[case::one_exiled("Source", &["A"], 1)]
    #[case::nothing_exiled("Source", &[], 0)]
    #[case::the_source_exiles_itself("Source", &["Source"], 0)]
    #[case::a_card_in_hand_is_no_permanent("H", &["A"], 0)]
    fn a_permanent_remembers_what_its_effects_exile(
        #[case] source: &str,
        #[case] exiled: &[&str],
        #[case] events: usize,
    ) {
        let game = game();
        let moves: Vec<Event> = exiled
            .iter()
            .map(|name| Event::ZoneChanged {
                object: named(&game, name).id,
                from: Zone::Battlefield,
                to: Zone::Exile,
                cause: ZoneCause::Exiled,
            })
            .collect();
        let made = exiled_by(named(&game, source), &moves, &game);
        assert_eq!(usize::from(made.is_some()), events);
        if let Some(Event::Remembered(change)) = made {
            assert_eq!(change.slot, EXILED_SLOT);
            assert_eq!(change.objects.len(), exiled.len());
            assert!(change.objects.iter().all(|object| object.counter == 1));
        }
    }

    #[test]
    fn the_memory_is_hashed_and_a_replay_rebuilds_it() {
        let mut game = game();
        let before = stable_hash(&game);
        let mut replay = game.clone();
        remember(&mut game, "slot", &["A"], false);
        assert_ne!(before, stable_hash(&game));
        remember(&mut replay, "slot", &["A"], false);
        assert_eq!(stable_hash(&game), stable_hash(&replay));
        remember(&mut game, "slot", &[], true);
        assert_eq!(before, stable_hash(&game));
    }

    #[test]
    fn a_reference_reads_what_the_named_source_remembers() {
        let mut game = game();
        remember(&mut game, "slot", &["A", "B"], false);
        let source = named(&game, "Source");
        let scope = Scope::announcing(&game, ME, source);
        let slot = |name: &str| Ref::Remembered {
            on: Box::new(Ref::This),
            slot: name.to_owned(),
        };
        assert_eq!(scope.objects(&slot("slot")).unwrap().len(), 2);
        assert_eq!(scope.objects(&slot("other")).unwrap().len(), 0);
    }
}
