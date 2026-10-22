//! Goal: whole bot games at two, three and four seats with decks of the devour, casualty and offspring corpus cards finish and keep every invariant (zone changes, life, counters, the journal, the replay of the log equal to the live state), and the games really use the keywords: devour creatures sacrifice others and enter with +1/+1 counters, casualty spells are copied, offspring makes 1/1 token copies, and the same games repeat from their seed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_cards::value::CounterKind;
use mtg_engine::events::ZoneCause;
use mtg_engine::{Engine, Event, GameSetup, replay};
use mtg_testkit::devour_cards::{CASUALTY_CARDS, DEVOUR_CARDS, OFFSPRING_CARDS};
use mtg_testkit::{devour_fuzz_setup, play_game};
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
    devour_counters: usize,
    sacrifices: usize,
    casualty_copies: usize,
    offspring_tokens: usize,
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
                Event::TokenCreated(token)
                    if token.copy_of.is_some() && listed(OFFSPRING_CARDS, &token.name) =>
                {
                    self.offspring_tokens += 1;
                }
                // Nothing else in the deck puts a +1/+1 counter on a creature, so these are devour's.
                Event::CountersChanged(change)
                    if change.delta > 0 && change.counter == CounterKind::PlusOnePlusOne =>
                {
                    let Some(on) = change.on.object() else {
                        continue;
                    };
                    if listed(DEVOUR_CARDS, &name(on.id)) {
                        self.devour_counters += 1;
                    }
                }
                Event::SpellCopied(copy) if listed(CASUALTY_CARDS, &name(copy.of.id)) => {
                    self.casualty_copies += 1;
                }
                Event::ZoneChanged {
                    cause: ZoneCause::Sacrificed,
                    ..
                } => self.sacrifices += 1,
                _ => {}
            }
        }
    }
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_devour_casualty_and_offspring_finish_and_keep_every_invariant(
    #[case] seats: usize,
) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = devour_fuzz_setup(seats, seed).unwrap();
        let outcome = play_game(setup.clone(), seed)
            .unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        seen.add(&setup, &outcome.events);
    }
    assert!(
        seen.devour_counters > 0,
        "a devour creature entered with +1/+1 counters"
    );
    assert!(seen.sacrifices > 0, "creatures were sacrificed");
    assert!(seen.casualty_copies > 0, "a casualty spell was copied");
    assert!(seen.offspring_tokens > 0, "offspring made a token copy");
}

#[rstest]
#[case::two(2, 21)]
#[case::four(4, 22)]
fn a_game_of_devour_casualty_and_offspring_repeats_exactly_from_its_seed(
    #[case] seats: usize,
    #[case] seed: u64,
) {
    let play = || {
        let setup = devour_fuzz_setup(seats, seed).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
    assert_eq!(first.final_hash, second.final_hash);
    let setup = devour_fuzz_setup(seats, seed).unwrap();
    let replayed = replay(&setup, &first.events).unwrap();
    assert_eq!(replayed.state_hash(), first.final_hash);
}

#[test]
fn the_cards_of_the_deck_are_all_in_the_corpus() {
    for &(name, _) in DEVOUR_CARDS
        .iter()
        .chain(CASUALTY_CARDS)
        .chain(OFFSPRING_CARDS)
    {
        assert!(mtg_testkit::corpus::card(name).is_ok(), "{name}");
    }
}
