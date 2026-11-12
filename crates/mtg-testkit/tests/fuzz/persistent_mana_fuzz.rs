//! Goal: whole bot games with decks of the creatures that add mana kept to the end of the turn (Savage Ventmaw, Colossal Plow, Klauth, Photon) beside lands, creatures and spells to spend it on finish and keep every invariant at tables of two, three and four seats, whether the bots pay automatically, exactly or in part, and the games really add kept mana and empty it with the turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::{for_each_seed, fuzz_games};

use std::sync::{Arc, Mutex};

use mtg_cards::CardDefinition;
use mtg_cards::spend::ManaLasts;
use mtg_engine::format::Normal;
use mtg_engine::{Event, GameSetup};
use mtg_testkit::cards::mountain;
use mtg_testkit::corpus::card;
use mtg_testkit::{PlayOptions, play_game_with};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(12)
}

/// Sixty cards of the corpus: the creatures that keep their mana beside lands, creatures and spells to pay for.
fn deck() -> Vec<Arc<CardDefinition>> {
    [
        ("Forest", 10),
        ("Savage Ventmaw", 4),
        ("Colossal Plow", 4),
        ("Klauth, Unrivaled Ancient", 3),
        ("Photon, Mighty Marvel", 4),
        ("Sol Ring", 2),
        ("Grizzly Bears", 4),
        ("Llanowar Elves", 3),
        ("Batterskull", 2),
        ("Bonesplitter", 3),
        ("Blaze", 3),
        ("Giant Growth", 2),
        ("Lightning Bolt", 3),
    ]
    .into_iter()
    .flat_map(|(name, copies)| (0..copies).map(move |_| card(name).unwrap()))
    .chain((0..13).map(|_| mountain()))
    .collect()
}

/// What a campaign saw.
#[derive(Default)]
struct Seen {
    kept: u64,
    emptied_with_the_turn: u64,
}

fn tally(events: &[Event], seen: &mut Seen) {
    for event in events {
        match event {
            Event::ManaAdded { lasts, .. } if *lasts != ManaLasts::Step => seen.kept += 1,
            Event::ManaPoolEmptied {
                through: ManaLasts::Turn,
                ..
            } => seen.emptied_with_the_turn += 1,
            _ => {}
        }
    }
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_kept_mana_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let decks = (0..players).map(|_| deck()).collect();
        let setup = GameSetup::new(Arc::new(Normal), seed, decks);
        let options = PlayOptions {
            partial_payments: seed % 2 == 0,
            symbol_choices: seed % 3 == 0,
            ..PlayOptions::default()
        };
        let outcome =
            play_game_with(setup, seed, options).unwrap_or_else(|failure| panic!("{failure}"));
        tally(&outcome.events, &mut seen.lock().unwrap());
    });
    let seen = seen.into_inner().unwrap();
    assert!(
        seen.kept > 0,
        "the games should add mana that is kept: {}",
        seen.kept
    );
    assert!(
        seen.emptied_with_the_turn > 0,
        "the kept mana should go with a turn: {}",
        seen.emptied_with_the_turn
    );
}

#[test]
fn a_seeded_game_with_kept_mana_is_the_same_game_every_time() {
    let play = |seed: u64| {
        let setup = GameSetup::new(Arc::new(Normal), seed, vec![deck(), deck()]);
        play_game_with(setup, seed, PlayOptions::default())
            .unwrap_or_else(|failure| panic!("{failure}"))
    };
    let (first, second) = (play(5), play(5));
    assert_eq!(first.final_hash, second.final_hash);
    assert_eq!(first.events, second.events);
}
