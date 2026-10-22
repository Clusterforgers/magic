//! Goal: whole bot games at two, three and four seats with decks of the characteristic and combat keyword cards (changeling, devoid, infect, wither, toxic, fear, intimidate, skulk, shadow, landwalk) finish, keep every invariant (life, damage, counters and poison accounted for through the independent model of the checker), deal infect and wither damage and toxic poison in some game, are the same game every time and replay to the same state, and every card of the deck is in the corpus.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use std::collections::BTreeSet;

use mtg_engine::combat::DamageForm;
use mtg_engine::{Event, replay};
use mtg_testkit::combat_keyword_cards::COMBAT_KEYWORD_CARDS;
use mtg_testkit::corpus::card;
use mtg_testkit::{combat_keywords_fuzz_setup, play_game};
use rstest::rstest;

fn games() -> u64 {
    fuzz_games(30)
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_combat_keyword_games_finish_keep_every_invariant_and_use_the_keywords(
    #[case] players: usize,
) {
    let games = games();
    let mut seen = BTreeSet::new();
    for seed in 0..games {
        let setup = combat_keywords_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        for event in &outcome.events {
            if let Event::DamageDealt { form, toxic, .. } = event {
                match form {
                    DamageForm::Infect => seen.insert("infect"),
                    DamageForm::Wither => seen.insert("wither"),
                    DamageForm::Normal => false,
                };
                if *toxic > 0 {
                    seen.insert("toxic");
                }
            }
        }
    }
    assert!(
        seen.contains("infect") && seen.contains("wither") && seen.contains("toxic"),
        "{games} games of {players} seats dealt only {seen:?}"
    );
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn a_seeded_combat_keyword_game_is_the_same_game_every_time_and_replays_to_its_state(
    #[case] players: usize,
) {
    for seed in 0..6u64 {
        let setup = combat_keywords_fuzz_setup(players, seed).unwrap();
        let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let second = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        assert_eq!(first.events, second.events, "seed {seed}");
        assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
        let state = replay(&setup, &first.events).unwrap();
        assert_eq!(state.state_hash(), first.final_hash, "seed {seed}");
    }
}

#[test]
fn the_cards_of_a_combat_keyword_deck_are_all_in_the_corpus() {
    for &(name, _) in COMBAT_KEYWORD_CARDS {
        assert!(card(name).is_ok(), "{name}");
    }
}
