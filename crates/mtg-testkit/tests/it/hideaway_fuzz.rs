//! Goal: bot games with the hideaway cards finish at two, three and four seats, replay exactly from their seed, and really hide cards, play lands and cast spells from the hiding, with every hidden card still in exile face down while its permanent stays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common::seeds::fuzz_games;
use mtg_core::ids::ObjectId;
use mtg_engine::Event;
use mtg_engine::events::ZoneCause;
use mtg_testkit::{hideaway_fuzz_setup, play_game};
use rstest::rstest;

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_hideaway_hide_cards_and_play_them(#[case] seats: usize) {
    let (mut hidden_cards, mut played_lands, mut cast_spells) = (0, 0, 0);
    for seed in 0..fuzz_games(60) {
        let setup = hideaway_fuzz_setup(seats, seed).unwrap();
        let outcome =
            play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        let mut hidden: BTreeSet<ObjectId> = BTreeSet::new();
        for event in &outcome.events {
            match event {
                Event::ZoneChanged {
                    object,
                    cause: ZoneCause::Hideaway { .. },
                    ..
                } => {
                    hidden.insert(*object);
                    hidden_cards += 1;
                }
                Event::LandPlayed { object, .. } if hidden.remove(object) => played_lands += 1,
                Event::SpellCast { object, .. } if hidden.remove(object) => cast_spells += 1,
                _ => {}
            }
        }
    }
    assert!(hidden_cards > 0, "a card was hidden");
    assert!(
        played_lands + cast_spells > 0,
        "a hidden card was played ({played_lands} lands, {cast_spells} spells)"
    );
}

#[rstest]
#[case::two(2, 41)]
#[case::four(4, 42)]
fn a_game_with_hideaway_repeats_exactly_from_its_seed(#[case] seats: usize, #[case] seed: u64) {
    let play = || {
        let setup = hideaway_fuzz_setup(seats, seed).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
}
