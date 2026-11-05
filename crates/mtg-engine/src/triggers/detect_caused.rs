//! Goal: `TriggerPattern::Caused` matches an event only when a spell or ability caused it, by the controller of the spell or ability that is resolving, is being cast or activated, or of the source of damage; the actions of the turn, state-based actions and every other controller never match.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::detect;
use super::detect_events::{FOE, ME, board, damage, fires, id, target};
use crate::abilities::PutOnStack;
use crate::choose::Choices;
use crate::combat::DamageTarget;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::resolve::Resolving;
use crate::snapshot::snapshot;
use mtg_cards::value::CounterKind;

/// What is running when the counters are put.
#[derive(Clone, Copy)]
enum Running {
    Nothing,
    Resolving(&'static str),
    Activating(PlayerId),
    Damage(&'static str),
}

fn counters(state: &crate::state::GameState, on: &str) -> Event {
    Event::CountersChanged(CountersChanged {
        on: target(state, on),
        counter: CounterKind::PlusOnePlusOne,
        delta: 1,
    })
}

fn caused(when: &str, running: Running) -> usize {
    let mut pre = board(when);
    let mut events = Vec::new();
    match running {
        Running::Nothing => {}
        Running::Resolving(name) => {
            let spell = id(&pre, name);
            pre.resolving = Some(Resolving::begin(spell));
        }
        Running::Activating(controller) => {
            let source = pre.object(id(&pre, "Watcher")).unwrap().reference();
            events.push(Event::AbilityPutOnStack(Box::new(PutOnStack {
                origin: crate::abilities::AbilitySource::default(),
                object: pre.next_object(),
                source,
                snapshot: snapshot(&pre, source.id).unwrap(),
                ability: 0,
                controller,
                choices: Choices::default(),
                cause: None,
                delayed: None,
            })));
        }
        Running::Damage(name) => events.push(damage(&pre, name, DamageTarget::Player(FOE))),
    }
    events.push(counters(&pre, "Ogre"));
    fires(&pre, &events).len()
}

const BY_YOU: &str = r#"{"Caused":{"inner":{"CountersPut":{}},"by":"You"}}"#;
const BY_THEM: &str = r#"{"Caused":{"inner":{"CountersPut":{}},"by":"Opponents"}}"#;
const BY_ANYONE: &str = r#"{"Caused":{"inner":{"CountersPut":{}}}}"#;
const BY_A_CREATURE: &str =
    r#"{"Caused":{"inner":{"CountersPut":{}},"source":[{"Type":"Creature"}]}}"#;
const BATCH_BY_YOU: &str = r#"{"Caused":{"inner":{"Batch":{"CountersPut":{}}},"by":"You"}}"#;

#[rstest]
#[case::my_spell_resolving(BY_YOU, Running::Resolving("Spell"), 1)]
#[case::their_spell_is_not_mine(BY_YOU, Running::Resolving("Their Spell"), 0)]
#[case::their_spell_is_an_opponents(BY_THEM, Running::Resolving("Their Spell"), 1)]
#[case::my_spell_is_not_an_opponents(BY_THEM, Running::Resolving("Spell"), 0)]
#[case::the_turn_has_no_cause(BY_YOU, Running::Nothing, 0)]
#[case::the_turn_has_no_cause_for_anyone(BY_ANYONE, Running::Nothing, 0)]
#[case::anyone_with_a_cause(BY_ANYONE, Running::Resolving("Their Spell"), 1)]
#[case::an_ability_activated_pays_its_costs(BY_YOU, Running::Activating(ME), 1)]
#[case::an_opponents_ability_activated(BY_YOU, Running::Activating(FOE), 0)]
#[case::the_source_of_damage_is_the_cause(BY_YOU, Running::Damage("Bear"), 1)]
#[case::an_opponents_source_of_damage(BY_YOU, Running::Damage("Ogre"), 0)]
#[case::a_creature_caused_it(BY_A_CREATURE, Running::Activating(ME), 1)]
#[case::a_spell_is_no_creature(BY_A_CREATURE, Running::Resolving("Spell"), 0)]
#[case::a_batch_of_counters(BATCH_BY_YOU, Running::Resolving("Spell"), 1)]
#[case::a_batch_with_no_cause(BATCH_BY_YOU, Running::Nothing, 0)]
fn an_event_matches_only_the_cause_the_pattern_names(
    #[case] when: &str,
    #[case] running: Running,
    #[case] expected: usize,
) {
    assert_eq!(caused(when, running), expected);
}

#[test]
fn a_cause_never_leaks_into_the_state_it_was_read_from() {
    let pre = board(BY_YOU);
    assert_eq!(detect(&pre, &[counters(&pre, "Ogre")], &pre).len(), 0);
}
