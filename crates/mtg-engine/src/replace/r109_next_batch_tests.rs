//! Goal: a shield used up by its next batch (`ReplaceBy::NextBatch`) changes every entering of the first batch it covers,
//! two creatures entering together both, and then ends: an entering of a later batch is left alone, and an entering it
//! does not cover does not use it up.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::modify::Duration;
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};
use mtg_core::card::CardType;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use rstest::rstest;
use std::sync::Arc;

use super::{Processed, process};
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::fold::apply_event;
use crate::rules::{Rule, RuleEffect};
use crate::state::GameState;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

/// Seat 0 holds a Rat, a Cat and a Forest in hand and controls a Bear; the other seats control nothing.
fn table(seats: usize) -> GameState {
    let forest = Arc::new(mtg_cards::CardDefinition::new("Forest").with_type(CardType::Land));
    let mut hands = vec![vec![creature("Rat", 1, 1), creature("Cat", 1, 1), forest]];
    hands.resize(seats, Vec::new());
    let mut fields = vec![vec![creature("Bear", 2, 2)]];
    fields.resize(seats, Vec::new());
    state(hands, fields)
}

fn id(game: &GameState, name: &str) -> ObjectId {
    game.objects()
        .find(|o| o.card().name == name)
        .map(crate::object::GameObject::id)
        .unwrap()
}

/// "The next time one or more creatures enter this turn, each enters with two +1/+1 counters on it", made by the Bear.
fn begin(game: &mut GameState) -> EffectId {
    let bear = game.object(id(game, "Bear")).unwrap();
    let effect = RuleEffect {
        id: EffectId(game.next_effect),
        rule: Rule::Replaces {
            when: serde_json::from_str::<ReplaceWhen>(r#"{"Enters":{"what":[{"Type":"Creature"}]}}"#)
                .unwrap(),
            by: serde_json::from_str::<ReplaceBy>(
                r#"{"NextBatch":{"WithCounters":{"counter":"+1/+1","count":2}}}"#,
            )
            .unwrap(),
            players: Vec::new(),
            bound: Vec::new(),
            lki: Vec::new(),
        },
        duration: Duration::UntilEndOfTurn,
        source: Some(bear.reference()),
        controller: bear.controller(),
        turn: 1,
    };
    let made = effect.id;
    apply_event(game, &Event::RuleCreated { effect: effect.into() }).unwrap();
    made
}

/// The batch of the named cards entering from the hand together.
fn entering(game: &GameState, names: &[&str]) -> Vec<Event> {
    names
        .iter()
        .flat_map(|name| {
            let object = id(game, name);
            [
                Event::ZoneChanged {
                    object,
                    from: Zone::Hand(ME),
                    to: Zone::Battlefield,
                    cause: ZoneCause::Moved,
                },
                Event::Entered {
                    object,
                    tapped: false,
                    controller: ME,
                    chosen: None,
                },
            ]
        })
        .collect()
}

fn processed(game: &GameState, batch: Vec<Event>) -> Vec<Event> {
    match process(game, batch, &[]).unwrap() {
        Processed::Events(events) => events,
        Processed::Ask { .. } => panic!("nothing is asked"),
    }
}

fn countered(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, Event::CountersChanged(_)))
        .count()
}

fn expired(events: &[Event], made: EffectId) -> bool {
    events
        .iter()
        .any(|event| matches!(event, Event::RuleExpired { id } if *id == made))
}

#[rstest]
fn both_creatures_of_the_next_batch_get_counters_and_the_shield_ends(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut game = table(seats);
    let made = begin(&mut game);
    let events = processed(&game, entering(&game, &["Rat", "Cat"]));
    assert_eq!(countered(&events), 2, "{events:?}");
    assert!(expired(&events, made), "{events:?}");
    for event in &events {
        apply_event(&mut game, event).unwrap();
    }
    assert!(game.rule_effects.iter().all(|effect| effect.id != made));
}

#[rstest]
fn an_entering_the_shield_does_not_cover_leaves_it_in_force(#[values(2, 3, 4)] seats: usize) {
    let mut game = table(seats);
    let made = begin(&mut game);
    let events = processed(&game, entering(&game, &["Forest"]));
    assert_eq!(countered(&events), 0, "{events:?}");
    assert!(!expired(&events, made), "{events:?}");
    for event in &events {
        apply_event(&mut game, event).unwrap();
    }
    let events = processed(&game, entering(&game, &["Rat"]));
    assert_eq!(countered(&events), 1, "{events:?}");
}
