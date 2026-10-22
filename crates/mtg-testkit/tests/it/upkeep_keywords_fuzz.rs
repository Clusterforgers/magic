//! Goal: whole bot games at two, three and four seats with decks of echo, cumulative upkeep, undying and persist cards (the corpus cards of the four keywords, a creature with undying and persist, burn and removal that kills them, and small creatures to sacrifice) finish and keep every invariant (zone changes, life, counters, the journal, the replay of the log equal to the live state), and the games really use the keywords: echo and cumulative upkeep triggers, age counters, creatures that come back with a +1/+1 or a -1/-1 counter, and the same games repeat from their seed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_cards::value::CounterKind;
use mtg_engine::abilities::AbilitySource;
use mtg_engine::{Engine, Event, replay};
use mtg_testkit::upkeep_cards::UPKEEP_CARDS;
use mtg_testkit::{play_game, upkeep_fuzz_setup};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(40)
}

/// Cards of the deck by keyword.
const ECHO: &[&str] = &[
    "Pouncing Jaguar",
    "Goblin War Buggy",
    "Rakdos Headliner",
    "Skizzik Surger",
];
const AGE: &[&str] = &[
    "Uktabi Efreet",
    "Morinfen",
    "Phyrexian Soulgorger",
    "Firestorm Hellkite",
];

#[derive(Default)]
struct Seen {
    echo: usize,
    age_triggers: usize,
    age_counters: usize,
    undying_returns: usize,
    persist_returns: usize,
}

impl Seen {
    fn add(&mut self, setup: &mtg_engine::GameSetup, events: &[Event]) {
        let engine = Engine::new(setup.clone()).unwrap();
        let name = |id| {
            engine
                .state()
                .object(id)
                .map(|object| object.card().name.clone())
                .unwrap_or_default()
        };
        let mut entered = BTreeSet::new();
        for event in events {
            match event {
                Event::AbilityTriggered(trigger)
                    if matches!(trigger.origin, AbilitySource::PrintedKeyword(_)) =>
                {
                    let source = name(trigger.source.id);
                    if ECHO.contains(&source.as_str()) {
                        self.echo += 1;
                    }
                    if AGE.contains(&source.as_str()) {
                        self.age_triggers += 1;
                    }
                }
                Event::ZoneChanged {
                    object,
                    to: mtg_engine::zones::Zone::Battlefield,
                    ..
                } => {
                    entered.insert(*object);
                }
                Event::CountersChanged(change) if change.delta > 0 => {
                    let Some(on) = change.on.object() else {
                        continue;
                    };
                    match &change.counter {
                        CounterKind::Other(kind) if kind == "age" => self.age_counters += 1,
                        // A counter put on a card the moment it came back is the one undying or persist gives.
                        CounterKind::PlusOnePlusOne if entered.remove(&on.id) => {
                            self.undying_returns += 1;
                        }
                        CounterKind::MinusOneMinusOne if entered.remove(&on.id) => {
                            self.persist_returns += 1;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_the_upkeep_keywords_finish_and_keep_every_invariant(#[case] seats: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = upkeep_fuzz_setup(seats, seed).unwrap();
        let outcome = play_game(setup.clone(), seed)
            .unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        seen.add(&setup, &outcome.events);
    }
    assert!(seen.echo > 0, "echo triggered");
    assert!(seen.age_triggers > 0, "cumulative upkeep triggered");
    assert!(seen.age_counters > 0, "age counters were put on permanents");
    assert!(
        seen.undying_returns > 0,
        "a creature came back with a +1/+1 counter"
    );
    assert!(
        seen.persist_returns > 0,
        "a creature came back with a -1/-1 counter"
    );
}

#[rstest]
#[case::two(2, 11)]
#[case::four(4, 12)]
fn a_game_of_the_upkeep_keywords_repeats_exactly_from_its_seed(
    #[case] seats: usize,
    #[case] seed: u64,
) {
    let play = || {
        let setup = upkeep_fuzz_setup(seats, seed).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
    assert_eq!(first.final_hash, second.final_hash);
    let setup = upkeep_fuzz_setup(seats, seed).unwrap();
    let replayed = replay(&setup, &first.events).unwrap();
    assert_eq!(replayed.state_hash(), first.final_hash);
}

#[test]
fn the_cards_of_the_deck_are_all_in_the_corpus() {
    for &(name, _) in UPKEEP_CARDS {
        assert!(mtg_testkit::corpus::card(name).is_ok(), "{name}");
    }
}
