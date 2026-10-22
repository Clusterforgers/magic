//! Goal: whole bot games with cost increases and reductions in play (taxes, reductions that count creatures or hold on a turn, Thalia, Foundry Inspector, Fireball and X spells beside hybrid and Phyrexian costs) finish with every invariant held, whoever controls the modifier, with the bots paying in part and choosing how to pay symbols, and the log of each game replays to its final state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Mutex;

use mtg_engine::{Event, replay};
use mtg_testkit::cost_cards::MODIFIER_CARDS;
use mtg_testkit::{PlayOptions, cost_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

/// What the games showed: modifier permanents cast, Thalia and Foundry Inspector cast, Fireballs aimed at several
/// targets, and the spells cast in all.
#[derive(Default)]
struct Seen {
    modifiers: u64,
    corpus_modifiers: u64,
    wide_fireballs: u64,
    casts: u64,
}

#[test]
fn bot_games_with_cost_modifiers_keep_every_invariant_and_replay() {
    let seen = Mutex::new(Seen::default());
    let games = fuzz_games(40);
    for_each_seed(0..games, |seed| {
        for (players, options) in [
            (2, PlayOptions::default()),
            (
                4,
                PlayOptions {
                    partial_payments: true,
                    symbol_choices: true,
                    ..PlayOptions::default()
                },
            ),
        ] {
            let outcome = play_game_with(cost_fuzz_setup(players, seed), seed, options)
                .unwrap_or_else(|failure| panic!("{failure}"));
            let replayed = replay(&cost_fuzz_setup(players, seed), &outcome.events).unwrap();
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                let Event::SpellCast {
                    object, choices, ..
                } = event
                else {
                    continue;
                };
                seen.casts += 1;
                let Some(name) = replayed.object(*object).map(|o| o.card().name.clone()) else {
                    continue;
                };
                if MODIFIER_CARDS.contains(&name.as_str()) {
                    seen.modifiers += 1;
                }
                if name == "Thalia, Guardian of Thraben" || name == "Foundry Inspector" {
                    seen.corpus_modifiers += 1;
                }
                if name == "Fireball" && choices.targets.first().is_some_and(|t| t.len() > 1) {
                    seen.wide_fireballs += 1;
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    assert!(seen.casts > 0, "spells were cast");
    assert!(seen.modifiers > 0, "some modifier permanents were cast");
    assert!(
        seen.corpus_modifiers > 0,
        "some Thalia or Inspector was cast"
    );
    assert!(
        seen.wide_fireballs > 0,
        "some Fireball was aimed at more than one target"
    );
}
