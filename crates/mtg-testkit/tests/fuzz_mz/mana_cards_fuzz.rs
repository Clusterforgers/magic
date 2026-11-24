//! Goal: whole bot games with decks of the creatures whose mana may be spent only on some spells and abilities (a list of creature types, artifacts, a mana value or an X, spells from a graveyard, spells the caster does not own) beside creatures and spells to pay for finish and keep every invariant at tables of two, three and four seats, whether the bots pay automatically, exactly or in part, and the games really add and spend such mana.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::{for_each_seed, fuzz_games};

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
    fuzz_games(12)
}

/// Sixty cards of the corpus: the creatures with restricted mana beside lands, creatures, artifacts and spells to pay for.
fn deck() -> Vec<Arc<CardDefinition>> {
    [
        ("Forest", 17),
        ("Cormela, Glamour Thief", 2),
        ("Flamebraider", 3),
        ("Ixalli's Lorekeeper", 3),
        ("Master of Dark Rites", 2),
        ("Vedalken Engineer", 3),
        ("Thieving Varmint", 2),
        ("Troyan, Gutsy Explorer", 2),
        ("Lord of the Forsaken", 2),
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
    .chain((0..2).map(|_| island()))
    .collect()
}

/// What a campaign saw.
#[derive(Default)]
struct Seen {
    added: u64,
    spent: u64,
}

/// What a game's log shows of restricted mana: added, and spent on a spell or an ability.
fn tally(events: &[Event], seen: &mut Seen) {
    for event in events {
        match event {
            Event::ManaAdded { only, .. } if !only.is_empty() => seen.added += 1,
            Event::ManaSpent { restricted, .. } => seen.spent += restricted.len() as u64,
            _ => {}
        }
    }
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_restricted_creatures_finish_and_keep_every_invariant(#[case] players: usize) {
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
        seen.added > 0 && seen.spent > 0,
        "the games should use restricted mana: added {}, spent {}",
        seen.added,
        seen.spent
    );
}

#[test]
fn a_seeded_game_with_restricted_creatures_is_the_same_game_every_time() {
    let play = |seed: u64| {
        let setup = GameSetup::new(Arc::new(Normal), seed, vec![deck(), deck()]);
        play_game_with(setup, seed, PlayOptions::default())
            .unwrap_or_else(|failure| panic!("{failure}"))
    };
    let (first, second) = (play(5), play(5));
    assert_eq!(first.final_hash, second.final_hash);
    assert_eq!(first.events, second.events);
}
