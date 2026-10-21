//! Goal: whole bot games with decks of Sol Ring, Gaea's Cradle, a painland and Lotus Petal beside lands, creatures and spells finish and keep every invariant at tables of two, three and four seats, whether the bots pay automatically, exactly or in part, and the games really use these abilities: several mana added at once, damage after a painland's mana and a Lotus Petal sacrificed for its mana.
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

/// Sixty cards of the corpus: lands and mana sources of every shape with creatures and spells to pay for.
fn deck() -> Vec<Arc<CardDefinition>> {
    [
        ("Forest", 24),
        ("Battlefield Forge", 4),
        ("Sol Ring", 4),
        ("Gaea's Cradle", 2),
        ("Lotus Petal", 4),
        ("Llanowar Elves", 4),
        ("Grizzly Bears", 4),
        ("Dockside Extortionist", 3),
        ("Giant Growth", 4),
        ("Lightning Bolt", 4),
        ("Counterspell", 3),
    ]
    .into_iter()
    .flat_map(|(name, copies)| (0..copies).map(move |_| card(name).unwrap()))
    .chain((0..4).map(|_| island()))
    .collect()
}

/// What a campaign saw.
#[derive(Default)]
struct Seen {
    several_at_once: u64,
    painland_damage: u64,
    petals_sacrificed: u64,
}

/// What a game's log shows of these abilities: mana added in a run by one source, damage right after mana, a sacrifice right before it.
fn tally(events: &[Event], seen: &mut Seen) {
    for pair in events.windows(2) {
        match (&pair[0], &pair[1]) {
            (
                Event::ManaAdded {
                    source: Some(a), ..
                },
                Event::ManaAdded {
                    source: Some(b), ..
                },
            ) if a == b => seen.several_at_once += 1,
            (
                Event::ManaAdded {
                    source: Some(a), ..
                },
                Event::DamageDealt { source: b, .. },
            ) if a == b => seen.painland_damage += 1,
            (
                Event::ZoneChanged {
                    object,
                    cause: mtg_engine::events::ZoneCause::Sacrificed,
                    ..
                },
                Event::ManaAdded {
                    source: Some(b), ..
                },
            ) if object == b => seen.petals_sacrificed += 1,
            _ => {}
        }
    }
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_mana_abilities_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let decks = (0..players).map(|_| deck()).collect();
        let setup = GameSetup::new(Arc::new(Normal), seed, decks);
        let options = PlayOptions {
            partial_payments: seed % 2 == 0,
            ..PlayOptions::default()
        };
        let outcome =
            play_game_with(setup, seed, options).unwrap_or_else(|failure| panic!("{failure}"));
        tally(&outcome.events, &mut seen.lock().unwrap());
    });
    let seen = seen.into_inner().unwrap();
    assert!(
        seen.several_at_once > 0 && seen.painland_damage > 0 && seen.petals_sacrificed > 0,
        "the games should use the abilities: several {}, pain {}, petals {}",
        seen.several_at_once,
        seen.painland_damage,
        seen.petals_sacrificed
    );
}

#[test]
fn a_seeded_game_with_mana_abilities_is_the_same_game_every_time() {
    let play = |seed: u64| {
        let setup = GameSetup::new(Arc::new(Normal), seed, vec![deck(), deck()]);
        play_game_with(setup, seed, PlayOptions::default())
            .unwrap_or_else(|failure| panic!("{failure}"))
    };
    let (first, second) = (play(5), play(5));
    assert_eq!(first.final_hash, second.final_hash);
    assert_eq!(first.events, second.events);
}
