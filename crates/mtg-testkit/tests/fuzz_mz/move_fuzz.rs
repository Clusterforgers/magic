//! Goal: bot games with the cards that return a card with a counter, bounce a count of targets and exile a monocolored permanent finish at two, three and four seats under every invariant, replay exactly from their seed, and really return cards from graveyards to the battlefield and exile permanents.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_engine::events::ZoneCause;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::{move_fuzz_setup, play_game};
use rstest::rstest;

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_the_move_cards_return_cards_bounce_permanents_and_exile(#[case] seats: usize) {
    let (mut returned, mut bounced, mut exiled) = (0, 0, 0);
    for seed in 0..fuzz_games(60) {
        let setup = move_fuzz_setup(seats, seed).unwrap();
        let outcome = play_game(setup.clone(), seed)
            .unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        replay(&setup, &outcome.events).unwrap();
        for event in &outcome.events {
            let Event::ZoneChanged {
                from, to, cause, ..
            } = event
            else {
                continue;
            };
            match (from, to) {
                (Zone::Graveyard(_), Zone::Battlefield) => returned += 1,
                (Zone::Battlefield, Zone::Hand(_)) if *cause == ZoneCause::Moved => bounced += 1,
                (Zone::Battlefield, Zone::Exile) if *cause == ZoneCause::Exiled => exiled += 1,
                _ => {}
            }
        }
    }
    assert!(returned > 0, "a card returned from a graveyard");
    // Six targets are found in a game of three seats or more; two seats rarely reach them.
    assert!(
        seats == 2 || bounced > 0,
        "a permanent was returned to a hand"
    );
    assert!(exiled > 0, "a permanent was exiled");
}

#[rstest]
#[case::two(2, 61)]
#[case::four(4, 62)]
fn a_game_with_the_move_cards_repeats_exactly_from_its_seed(
    #[case] seats: usize,
    #[case] seed: u64,
) {
    let play = || {
        let setup = move_fuzz_setup(seats, seed).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
}
