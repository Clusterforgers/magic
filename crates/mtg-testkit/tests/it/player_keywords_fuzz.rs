//! Goal: whole bot games with decks of the cards that give their controller hexproof or shroud finish and keep every invariant at two, three and four seats, the cards are cast in the campaign, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Mutex;

use mtg_engine::{Event, GameState, replay};
use mtg_testkit::player_keyword_cards::PLAYER_KEYWORD_CARDS;
use mtg_testkit::{play_game, player_keyword_fuzz_setup};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(40)
}

#[derive(Default)]
struct Seen {
    entered: BTreeMap<String, u64>,
}

impl Seen {
    /// The cards that resolved in the events, judged against the state at the end of the game.
    fn add(&mut self, game: &GameState, events: &[Event]) {
        for event in events {
            if let Event::SpellResolved { object } = event
                && let Some(object) = game.object(*object)
            {
                *self.entered.entry(object.card().name.clone()).or_default() += 1;
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_player_keyword_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let setup = player_keyword_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.lock().unwrap().add(&end, &outcome.events);
    });
    let seen = seen.into_inner().unwrap();
    let campaign = format!("{} games of {players} seats", games());
    let entered = PLAYER_KEYWORD_CARDS
        .iter()
        .filter(|name| seen.entered.get(**name).copied().unwrap_or(0) > 0)
        .count();
    assert!(
        entered * 2 >= PLAYER_KEYWORD_CARDS.len(),
        "only {entered} of {} cards were cast in {campaign}",
        PLAYER_KEYWORD_CARDS.len()
    );
}

#[test]
fn a_seeded_player_keyword_game_is_the_same_game_every_time() {
    let seed = 3;
    let setup = player_keyword_fuzz_setup(2, seed).unwrap();
    let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
    let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
    assert_eq!(first.events, second.events, "seed {seed}");
}
