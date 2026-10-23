//! Goal: whole bot games of Commander at 2 to 4 seats whose decks hold the cards that read the commander cast count, the once-a-turn limit and the subtype, permanent, multicolored and historic cast triggers finish and keep every invariant (zones, life, the journal, the replay of the log equal to the live state), those cards are cast and their abilities trigger, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::{for_each_seed, fuzz_games};

use std::collections::BTreeSet;
use std::sync::Mutex;

use mtg_engine::{Event, GameSetup, GameState, apply_event};
use mtg_testkit::{CAST_KIND_CARDS, cast_kind_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(6)
}

/// What a campaign did with the cards.
#[derive(Default, Debug)]
struct Seen {
    /// The names of the cards of the deck that were cast.
    cast: BTreeSet<String>,
    /// The abilities of those cards that went on the stack.
    triggered: u64,
}

impl Seen {
    /// Counts the casts and the abilities of the cards by folding the log over its setup.
    fn add(&mut self, setup: &GameSetup, events: &[Event]) {
        let mut state = GameState::from_setup(setup).unwrap();
        for event in events {
            let source = match event {
                Event::SpellCast { object, .. } => Some((*object, true)),
                Event::AbilityPutOnStack(put) if put.cause.is_some() => {
                    Some((put.source.id, false))
                }
                _ => None,
            };
            if let Some((object, cast)) = source
                && let Some(card) = state.object(object)
                && CAST_KIND_CARDS.contains(&card.card().name.as_str())
            {
                if cast {
                    self.cast.insert(card.card().name.clone());
                } else {
                    self.triggered += 1;
                }
            }
            apply_event(&mut state, event).unwrap();
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_games_with_the_cast_kind_cards_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let setup = cast_kind_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.lock().unwrap().add(&setup, &outcome.events);
    });
    let seen = seen.into_inner().unwrap();
    eprintln!("{players} seats: {seen:?}");
    assert!(!seen.cast.is_empty(), "no card was cast: {seen:?}");
    assert!(seen.triggered > 0, "no ability triggered: {seen:?}");
}

#[test]
fn a_seeded_game_with_the_cast_kind_cards_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(cast_kind_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        let second = play_game(cast_kind_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
