//! Goal: whole bot games at two, three and four seats with decks of the triggered keywords (the corpus cards with exalted, melee, mentor, dethrone, annihilator, extort and evolve, a cheap annihilator creature, burn and small creatures) finish and keep every invariant (zone changes, life, counters, the journal, the replay of the log equal to the live state), and the games really use the keywords: every keyword triggers, permanents are sacrificed after annihilator, +1/+1 counters are put on creatures, and the same games repeat from their seed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;

use mtg_cards::value::CounterKind;
use mtg_engine::abilities::AbilitySource;
use mtg_engine::events::ZoneCause;
use mtg_engine::{Engine, Event, replay};
use mtg_testkit::triggered_cards::TRIGGERED_CARDS;
use mtg_testkit::{play_game, triggered_fuzz_setup};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(60)
}

/// The cards of the deck by keyword.
const KEYWORDS: &[(&str, &[&str])] = &[
    (
        "exalted",
        &[
            "Akrasan Squire",
            "Aven Squire",
            "Sigiled Paladin",
            "Rhox Charger",
        ],
    ),
    (
        "extort",
        &[
            "Basilica Screecher",
            "Tithe Drinker",
            "Syndic of Tithes",
            "Kingpin's Pet",
        ],
    ),
    (
        "evolve",
        &[
            "Cloudfin Raptor",
            "Crocanura",
            "Adaptive Snapjaw",
            "Battering Krasis",
        ],
    ),
    (
        "mentor",
        &["Sunhome Stalwart", "Parhelion Patrol", "Hammer Dropper"],
    ),
    ("melee", &["Wings of the Guard", "Deputized Protester"]),
    (
        "dethrone",
        &[
            "Grenzo's Cutthroat",
            "Enraged Revolutionary",
            "Marchesa's Emissary",
        ],
    ),
    ("annihilator", &["Ulamog's Crusher", "Triggered Devourer"]),
];

#[derive(Default)]
struct Seen {
    triggers: [usize; KEYWORDS.len()],
    sacrifices: usize,
    counters: usize,
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
        for event in events {
            match event {
                Event::AbilityTriggered(trigger)
                    if matches!(trigger.origin, AbilitySource::PrintedKeyword(_)) =>
                {
                    let source = name(trigger.source.id);
                    for (at, (_, cards)) in KEYWORDS.iter().enumerate() {
                        if cards.contains(&source.as_str()) {
                            self.triggers[at] += 1;
                        }
                    }
                }
                Event::ZoneChanged {
                    cause: ZoneCause::Sacrificed,
                    ..
                } => self.sacrifices += 1,
                Event::CountersChanged(change)
                    if change.delta > 0 && change.counter == CounterKind::PlusOnePlusOne =>
                {
                    self.counters += 1;
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
fn bot_games_with_the_triggered_keywords_finish_and_keep_every_invariant(#[case] seats: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = triggered_fuzz_setup(seats, seed).unwrap();
        let outcome = play_game(setup.clone(), seed)
            .unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        seen.add(&setup, &outcome.events);
    }
    for (at, (keyword, _)) in KEYWORDS.iter().enumerate() {
        assert!(seen.triggers[at] > 0, "{keyword} triggered");
    }
    assert!(seen.sacrifices > 0, "annihilator made a player sacrifice");
    assert!(seen.counters > 0, "+1/+1 counters were put on creatures");
}

#[rstest]
#[case::two(2, 11)]
#[case::four(4, 12)]
fn a_game_of_the_triggered_keywords_repeats_exactly_from_its_seed(
    #[case] seats: usize,
    #[case] seed: u64,
) {
    let play = || {
        let setup = triggered_fuzz_setup(seats, seed).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
    assert_eq!(first.final_hash, second.final_hash);
    let setup = triggered_fuzz_setup(seats, seed).unwrap();
    let replayed = replay(&setup, &first.events).unwrap();
    assert_eq!(replayed.state_hash(), first.final_hash);
}

#[test]
fn the_cards_of_the_deck_are_all_in_the_corpus() {
    for &(name, _) in TRIGGERED_CARDS {
        assert!(mtg_testkit::corpus::card(name).is_ok(), "{name}");
    }
}
