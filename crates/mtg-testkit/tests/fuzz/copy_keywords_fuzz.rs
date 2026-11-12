//! Goal: whole bot games at two, three and four seats with decks of the myriad, squad and ravenous corpus cards finish and keep every invariant (zone changes, life, counters, the journal, the replay of the log equal to the live state), and the games really use the keywords: tapped token copies enter attacking at tables of three and four and never at a table of two, token copies enter from squad, creatures enter with +1/+1 counters from ravenous, and the same games repeat from their seed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_cards::value::CounterKind;
use mtg_engine::{Engine, Event, GameSetup, replay};
use mtg_testkit::copy_keyword_cards::{MYRIAD_CARDS, RAVENOUS_CARDS, SQUAD_CARDS};
use mtg_testkit::{copy_keywords_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(60)
}

fn listed(list: &[(&str, usize)], name: &str) -> bool {
    list.iter().any(|(listed, _)| *listed == name)
}

#[derive(Default)]
struct Seen {
    myriad_tokens: usize,
    squad_tokens: usize,
    ravenous_counters: usize,
}

impl Seen {
    fn add(&mut self, setup: &GameSetup, events: &[Event]) {
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
                Event::TokenCreated(token) if token.copy_of.is_some() => {
                    if token.attacking.is_some() && token.tapped {
                        self.myriad_tokens += 1;
                    } else if listed(SQUAD_CARDS, &token.name) {
                        self.squad_tokens += 1;
                    }
                }
                // Nothing else in the deck puts a counter on a creature, so these are the counters it enters with.
                Event::CountersChanged(change)
                    if change.delta > 0 && change.counter == CounterKind::PlusOnePlusOne =>
                {
                    let Some(on) = change.on.object() else {
                        continue;
                    };
                    if listed(RAVENOUS_CARDS, &name(on.id)) {
                        self.ravenous_counters += 1;
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
fn bot_games_with_the_copy_keywords_finish_and_keep_every_invariant(#[case] seats: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = copy_keywords_fuzz_setup(seats, seed).unwrap();
        let outcome = play_game(setup.clone(), seed)
            .unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        seen.add(&setup, &outcome.events);
    }
    if seats == 2 {
        assert_eq!(seen.myriad_tokens, 0, "no other opponent to attack");
    } else {
        assert!(seen.myriad_tokens > 0, "a token copy entered attacking");
    }
    assert!(seen.squad_tokens > 0, "squad made token copies");
    assert!(
        seen.ravenous_counters > 0,
        "a creature entered with +1/+1 counters"
    );
}

#[rstest]
#[case::two(2, 11)]
#[case::four(4, 12)]
fn a_game_of_the_copy_keywords_repeats_exactly_from_its_seed(
    #[case] seats: usize,
    #[case] seed: u64,
) {
    let play = || {
        let setup = copy_keywords_fuzz_setup(seats, seed).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
    assert_eq!(first.final_hash, second.final_hash);
    let setup = copy_keywords_fuzz_setup(seats, seed).unwrap();
    let replayed = replay(&setup, &first.events).unwrap();
    assert_eq!(replayed.state_hash(), first.final_hash);
}

#[test]
fn the_cards_of_the_deck_are_all_in_the_corpus() {
    for &(name, _) in MYRIAD_CARDS.iter().chain(SQUAD_CARDS).chain(RAVENOUS_CARDS) {
        assert!(mtg_testkit::corpus::card(name).is_ok(), "{name}");
    }
}
