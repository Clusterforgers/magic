//! Goal: whole bot games with decks of lands that add restricted mana (for creatures, for artifacts, for multicolored spells, for costs with X) beside creatures, artifacts and spells finish and keep every invariant at tables of two, three and four seats, whether the bots pay automatically, exactly or in part, and the games really spend restricted mana: it is added, it is spent on a spell, and some of it floats unspent.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::for_each_seed;

use std::sync::{Arc, Mutex};

use mtg_cards::CardDefinition;
use mtg_engine::format::Normal;
use mtg_engine::{Event, GameSetup};
use mtg_testkit::cards::island;
use mtg_testkit::corpus::card;
use mtg_testkit::{PlayOptions, play_game_with};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40)
}

/// Sixty cards of the corpus: lands that add restricted mana beside plain ones, with creatures, artifacts, a multicolored
/// spell and a spell with X to pay for.
fn deck() -> Vec<Arc<CardDefinition>> {
    [
        ("Forest", 17),
        ("Ancient Ziggurat", 4),
        ("Mishra's Workshop", 4),
        ("Pillar of the Paruns", 4),
        ("Rosheen Meanderer", 3),
        ("Sol Ring", 3),
        ("Lotus Petal", 2),
        ("Grizzly Bears", 4),
        ("Llanowar Elves", 3),
        ("Batterskull", 2),
        ("Bonesplitter", 3),
        ("Blaze", 3),
        ("Giant Growth", 3),
        ("Lightning Bolt", 3),
    ]
    .into_iter()
    .flat_map(|(name, copies)| (0..copies).map(move |_| card(name).unwrap()))
    .chain((0..2).map(|_| island()))
    .collect()
}

/// What a campaign saw.
#[derive(Default)]
struct Seen {
    added: u64,
    spent: u64,
    floated_away: u64,
}

/// What a game's log shows of restricted mana: added, spent on a spell or an ability, and gone with the pool.
fn tally(events: &[Event], seen: &mut Seen) {
    let mut held = 0u64;
    for event in events {
        match event {
            Event::ManaAdded { only, .. } if !only.is_empty() => {
                seen.added += 1;
                held += 1;
            }
            Event::ManaSpent { restricted, .. } => {
                seen.spent += restricted.len() as u64;
                held = held.saturating_sub(restricted.len() as u64);
            }
            Event::ManaPoolEmptied { .. } => {
                if held > 0 {
                    seen.floated_away += 1;
                }
                held = 0;
            }
            _ => {}
        }
    }
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_restricted_mana_finish_and_keep_every_invariant(#[case] players: usize) {
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
        seen.added > 0 && seen.spent > 0 && seen.floated_away > 0,
        "the games should use restricted mana: added {}, spent {}, lost with the pool {}",
        seen.added,
        seen.spent,
        seen.floated_away
    );
}

#[test]
fn a_seeded_game_with_restricted_mana_is_the_same_game_every_time() {
    let play = |seed: u64| {
        let setup = GameSetup::new(Arc::new(Normal), seed, vec![deck(), deck()]);
        play_game_with(setup, seed, PlayOptions::default())
            .unwrap_or_else(|failure| panic!("{failure}"))
    };
    let (first, second) = (play(5), play(5));
    assert_eq!(first.final_hash, second.final_hash);
    assert_eq!(first.events, second.events);
}
