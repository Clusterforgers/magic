//! Goal: whole bot games with decks of the cards that mill for mana (Millikin, The Warring Triad), count a hand (Rousing Refrain), remove X counters for mana (Haruspex) and add mana as a creature or the one it equips attacks (Tanuki Transplanter) beside lands, creatures and spells finish and keep every invariant at tables of two, three and four seats, the games really mill for mana, and a seeded game replays the same.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::{for_each_seed, fuzz_games};

use std::sync::{Arc, Mutex};

use mtg_cards::CardDefinition;
use mtg_engine::events::ZoneCause;
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

/// Sixty cards of the corpus: the cards that mill, count and remove counters beside lands, creatures and spells.
fn deck() -> Vec<Arc<CardDefinition>> {
    [
        ("Forest", 12),
        ("Millikin", 4),
        ("The Warring Triad", 2),
        ("Rousing Refrain", 2),
        ("Haruspex", 3),
        ("Tanuki Transplanter", 3),
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
    .chain((0..12).map(|_| mountain()))
    .collect()
}

/// What a campaign saw.
#[derive(Default)]
struct Seen {
    mana_added: u64,
    milled: u64,
}

/// Counts the mana permanents added and the cards milled.
fn tally(events: &[Event], seen: &mut Seen) {
    for event in events {
        match event {
            Event::ManaAdded {
                source: Some(_), ..
            } => seen.mana_added += 1,
            Event::ZoneChanged {
                cause: ZoneCause::Milled,
                ..
            } => seen.milled += 1,
            _ => {}
        }
    }
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_milling_mana_finish_and_keep_every_invariant(#[case] players: usize) {
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
        seen.mana_added > 0,
        "the games should add mana from permanents: {}",
        seen.mana_added
    );
    assert!(seen.milled > 0, "the games should mill: {}", seen.milled);
}

#[test]
fn a_seeded_game_with_milling_mana_is_the_same_game_every_time() {
    let play = |seed: u64| {
        let setup = GameSetup::new(Arc::new(Normal), seed, vec![deck(), deck()]);
        play_game_with(setup, seed, PlayOptions::default())
            .unwrap_or_else(|failure| panic!("{failure}"))
    };
    let (first, second) = (play(5), play(5));
    assert_eq!(first.final_hash, second.final_hash);
    assert_eq!(first.events, second.events);
}
