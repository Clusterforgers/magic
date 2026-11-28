//! Source memory: the objects a source remembers by slot (`Effect::Remember`, `Ref::Remembered`, `Pred::ExiledWith`).

use std::collections::BTreeMap;

use mtg_cards::refs::EXILED_SLOT;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use serde::{Deserialize, Serialize};

use crate::events::{Event, ZoneCause};
use crate::object::ObjectRef;
use crate::state::GameState;
use crate::zones::Zone;

/// The slot of a source's memory that holds the face-down pile it keeps (`PileFate::R406Kept`), in its shuffled order; a
/// card file cannot name it.
pub(crate) const R406_PILE_SLOT: &str = "~r406 pile";

/// The slot of a source's memory that holds the mana it noted, one number per mana (`ManaType::index`).
pub(crate) const R112_NOTED_SLOT: &str = "r112 noted mana";

/// What each source remembers: the things by slot, each object as the object it was when remembered (CR 400.7).
pub type Memory = BTreeMap<(ObjectRef, String), Vec<Held>>;

/// The prefix of a slot whose numbers outlast the resolution that wrote them: the numbers a permanent keeps for as long as
/// it stays (die results stored on it, the numbers it chose before); a card file cannot name such a slot.
pub const R112_LASTING: &str = "~kept:";

/// The lasting slot of a name (`R112_LASTING`).
pub(crate) fn r112_lasting(slot: &str) -> String {
    format!("{R112_LASTING}{slot}")
}

/// One thing a source remembers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Held {
    /// An object, as the incarnation it was.
    Object(ObjectRef),
    /// A player.
    Player(PlayerId),
    /// A number.
    Number(i64),
    /// A word noted (`Effect::R112NoteChosen`): a creature type.
    R112Word(String),
}

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
    /// The players and numbers added to the slot after the objects (`Effect::RememberValue`).
    #[serde(default)]
    pub values: Vec<Held>,
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
        let object = Held::Object(*object);
        if !held.contains(&object) {
            held.push(object);
        }
    }
    for value in &change.values {
        // A number may repeat (two remembered 3s sum to 6); an object or a player is held once.
        if matches!(value, Held::Number(_)) || !held.contains(value) {
            held.push(value.clone());
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
    if let Event::SpellResolved { object } = event {
        carry_exiled_with_cost(state, *object);
        // A permanent spell enters as it resolves (no `ZoneChanged` is logged).
        r113_carry_kept(state, *object);
    }
    if let Event::ZoneChanged {
        object,
        to: Zone::Exile,
        ..
    } = event
    {
        r113_note_exiled(state, *object);
    }
    if state.memory.is_empty() {
        return;
    }
    match event {
        Event::ZoneChanged { object, to, .. } => {
            let id = *object;
            if *to == Zone::Battlefield {
                r113_carry_kept(state, id);
            }
            let chosen = crate::replace::r114_choose_object::r114_chosen_slot();
            // The object chosen as a permanent entered stays named as it was (its copy outlives it).
            for (_, held) in state
                .memory
                .iter_mut()
                .filter(|((_, slot), _)| *slot != chosen)
            {
                held.retain(|named| !matches!(named, Held::Object(object) if object.id == id));
            }
            let forgotten = state.object(id).and_then(|left| {
                let me = left.reference();
                (!crate::ability_reader::has_triggered(left)
                    && !waiting(state, me, Waiting::default()))
                .then_some(me)
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
            // A number lives for the resolution that wrote it, so a later one never reads an earlier one's.
            let writer = source.or_else(|| {
                state
                    .object(*object)
                    .map(crate::object::GameObject::reference)
            });
            if let Some(writer) = writer {
                for ((owner, slot), held) in &mut state.memory {
                    // A note of spent mana stays until the next note replaces it (`r112_note`).
                    if *owner == writer
                        && slot != R112_NOTED_SLOT
                        && !slot.starts_with(R112_LASTING)
                    {
                        held.retain(|one| !matches!(one, Held::Number(_)));
                    }
                }
            }
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

/// The numbers stored as a permanent enters (`ReplaceBy::AsEnters`, CR 614.12: "As this creature enters, roll a
/// six-sided die twice") are the entering object's, so its lasting slots go with it to the permanent it becomes.
fn r113_carry_kept(state: &mut GameState, id: mtg_core::ids::ObjectId) {
    let Some(entering) = state.object(id).map(crate::object::GameObject::reference) else {
        return;
    };
    let arriving = ObjectRef {
        id,
        counter: entering.counter + 1,
    };
    let kept: Vec<String> = state
        .memory
        .keys()
        .filter(|(source, slot)| *source == entering && slot.starts_with(R112_LASTING))
        .map(|(_, slot)| slot.clone())
        .collect();
    for slot in kept {
        if let Some(held) = state.memory.remove(&(entering, slot.clone())) {
            state.memory.insert((arriving, slot), held);
        }
    }
}

/// The lasting slot of the mana values a permanent noted in `turn` (`Rule::R113NoteExiled`).
pub(crate) fn r113_noted_slot(turn: u32) -> String {
    r112_lasting(&format!("r113 noted in turn {turn}"))
}

/// "Note the mana value of each card as it's put into exile.": each permanent with the rule notes the mana value of the
/// card `id` as it goes, as it was before (no stack, no trigger); the notes of an earlier turn are dropped. A token is no
/// card and is not noted.
fn r113_note_exiled(state: &mut GameState, id: mtg_core::ids::ObjectId) {
    let Some(card) = state
        .object(id)
        .filter(|card| card.kind() == crate::object::ObjectKind::Card)
    else {
        return;
    };
    let value = i64::from(card.mana_value());
    let noters = crate::rules::r113_noters(state);
    if noters.is_empty() {
        return;
    }
    let turn = state.turn().number();
    let slot = r113_noted_slot(turn);
    let prefix = r112_lasting("r113 noted in turn ");
    for noter in noters {
        state.memory.retain(|(source, kept), _| {
            *source != noter || !kept.starts_with(&prefix) || *kept == slot
        });
        state
            .memory
            .entry((noter, slot.clone()))
            .or_default()
            .push(Held::Number(value));
    }
}

/// A permanent spell that exiled cards as it was cast (an additional cost) remembers them as exiled with it once it resolves
/// (CR 607.2a): the new object arrives with the counter after the one the spell has.
fn carry_exiled_with_cost(state: &mut GameState, id: mtg_core::ids::ObjectId) {
    let Some(spell) = state.object(id) else {
        return;
    };
    let types = spell.base_types();
    if types.contains(CardType::Instant) || types.contains(CardType::Sorcery) {
        return;
    }
    let arriving = ObjectRef {
        id,
        counter: spell.counter() + 1,
    };
    let exiled: Vec<ObjectRef> = spell
        .choices()
        .cost_picks()
        .iter()
        .flatten()
        .filter(|picked| **picked != id)
        .filter_map(|picked| state.object(*picked))
        .filter(|card| card.zone() == Zone::Exile)
        .map(crate::object::GameObject::reference)
        .collect();
    if !exiled.is_empty() {
        state.memory.insert(
            (arriving, EXILED_SLOT.to_owned()),
            exiled.into_iter().map(Held::Object).collect(),
        );
    }
}

/// The abilities that do not count as waiting because they are the one that is ending.
#[derive(Debug, Clone, Copy, Default)]
struct Waiting {
    /// The ability on the stack that is ending.
    stacked: Option<mtg_core::ids::ObjectId>,
    /// How many of the queued triggers of the source are ending.
    queued: usize,
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
    on_stack
        || queued > ending.queued
        || state
            .delayed()
            .iter()
            .any(|delayed| delayed.source == source)
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
            values: Vec::new(),
            replace: false,
        }))
    })
}

/// Whether the source remembers `object` in the slot as it was one zone change ago: the object left the zone it was
/// remembered in with its latest zone change.
pub(crate) fn p170_held_before(
    state: &GameState,
    source: ObjectRef,
    slot: &str,
    object: ObjectRef,
) -> bool {
    let Some(counter) = object.counter.checked_sub(1) else {
        return false;
    };
    let before = ObjectRef {
        id: object.id,
        counter,
    };
    state
        .memory
        .get(&(source, slot.to_owned()))
        .is_some_and(|held| held.contains(&Held::Object(before)))
}

/// The objects the source remembers in the slot that are still the objects remembered.
pub(crate) fn recall(state: &GameState, source: ObjectRef, slot: &str) -> Vec<ObjectRef> {
    state
        .memory
        .get(&(source, slot.to_owned()))
        .into_iter()
        .flatten()
        .filter_map(|held| match held {
            Held::Object(object) => Some(*object),
            _ => None,
        })
        .filter(|named| {
            state
                .object(named.id)
                .is_some_and(|object| object.counter() == named.counter)
        })
        .collect()
}

/// The players the source remembers in the slot who are still in the game.
pub(crate) fn recall_players(state: &GameState, source: ObjectRef, slot: &str) -> Vec<PlayerId> {
    state
        .memory
        .get(&(source, slot.to_owned()))
        .into_iter()
        .flatten()
        .filter_map(|held| match held {
            Held::Player(player) => Some(*player),
            _ => None,
        })
        .filter(|player| state.is_alive(*player))
        .collect()
}

/// The numbers the source remembers in the slot, in the order remembered.
/// The slot a number kept for one player lives in (`Effect::RememberValue` with `per`): the slot name and the player.
pub(crate) fn player_slot(slot: &str, player: PlayerId) -> String {
    format!("{slot}@{}", player.0)
}

pub(crate) fn recall_numbers(state: &GameState, source: ObjectRef, slot: &str) -> Vec<i64> {
    state
        .memory
        .get(&(source, slot.to_owned()))
        .into_iter()
        .flatten()
        .filter_map(|held| match held {
            Held::Number(number) => Some(*number),
            _ => None,
        })
        .collect()
}

/// The slot of the words a source noted (`Effect::R112NoteChosen`).
pub(crate) const R112_NOTED_WORDS_SLOT: &str = "~noted";

/// The words the source has noted, in the order it noted them.
pub(crate) fn r112_noted_words(state: &GameState, source: ObjectRef) -> Vec<String> {
    state
        .memory
        .get(&(source, R112_NOTED_WORDS_SLOT.to_owned()))
        .map(|held| {
            held.iter()
                .filter_map(|one| match one {
                    Held::R112Word(word) => Some(word.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
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
            values: Vec::new(),
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
