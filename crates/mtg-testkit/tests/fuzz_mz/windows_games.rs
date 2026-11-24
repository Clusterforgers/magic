//! Goal: bot games under the reference windows end like any other, the windows decide who is asked (no seat is passed for in Full Control, most windows are passed for under the defaults), a bot's choice of stops and mode is reproducible and well formed, and the same seed plays the same game.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_testkit::windows::Side;
use mtg_testkit::{PlayOptions, RandomBot, fuzz_setup, play_game_with};
use rstest::rstest;

fn options(auto_pass: bool, bot_windows: bool) -> PlayOptions {
    PlayOptions {
        auto_pass,
        bot_windows,
        ..PlayOptions::default()
    }
}

#[rstest]
fn games_end_under_every_rule_and_the_passes_depend_on_it(
    #[values(2, 3, 4)] players: usize,
    #[values(1, 2, 3, 4, 5)] seed: u64,
) {
    let play = |rule| {
        play_game_with(fuzz_setup(players, seed), seed, rule).unwrap_or_else(|f| panic!("{f}"))
    };
    let bare = play(options(false, false));
    let windows = play(options(true, false));
    let chosen = play(options(true, true));
    let held_by_default = play(options(false, true));

    assert_eq!(bare.auto_passes, 0, "every seat is asked everywhere");
    assert!(
        windows.auto_passes > windows.answers / 2,
        "the default windows pass for most of the table: {} of {}",
        windows.auto_passes,
        windows.answers
    );
    for outcome in [&bare, &windows, &chosen, &held_by_default] {
        assert!(outcome.auto_passes <= outcome.answers);
    }
}

#[rstest]
fn the_same_seed_plays_the_same_game(#[values(2, 4)] players: usize, #[values(1, 2, 3)] seed: u64) {
    for rule in [
        options(true, false),
        options(true, true),
        options(false, true),
    ] {
        let play = || {
            play_game_with(fuzz_setup(players, seed), seed, rule).unwrap_or_else(|f| panic!("{f}"))
        };
        let (first, second) = (play(), play());
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}

#[test]
fn a_bots_choice_is_reproducible_and_well_formed() {
    let mut modes = BTreeSet::new();
    let mut kept = 0;
    let mut one_turn = 0;
    for seed in 1..=40 {
        for stream in 0..4 {
            let chosen = RandomBot::new(seed, stream).windows();
            assert_eq!(
                chosen,
                RandomBot::new(seed, stream).windows(),
                "seed {seed} stream {stream}"
            );
            let places: Vec<_> = chosen
                .stops
                .iter()
                .map(|s| (s.side == Side::Own, s.step))
                .collect();
            let distinct: BTreeSet<_> = places.iter().copied().collect();
            assert_eq!(
                distinct.len(),
                places.len(),
                "one stop for each side and step"
            );
            assert!(chosen.stops.iter().all(|s| s.step.grants_priority()));
            kept += chosen.stops.iter().filter(|s| s.keep).count();
            one_turn += chosen.stops.iter().filter(|s| !s.keep).count();
            modes.insert(format!("{:?}", chosen.mode));
        }
    }
    assert_eq!(modes.len(), 3, "every mode turns up: {modes:?}");
    assert!(
        kept > 20 && one_turn > 20,
        "{kept} kept, {one_turn} one-turn"
    );
}
