//! Goal: whole bot games at two to four seats with the split second and undaunted corpus cards beside creatures with activated, mana and triggered abilities finish with every invariant held and replay to their final state; in every game no spell is cast and no ability that is not a mana ability is activated while a spell with split second is on the stack, and across the games such spells were cast, mana abilities and triggers went on inside their windows, and spells with undaunted were cast.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Mutex;

use mtg_cards::keyword::Keyword;
use mtg_engine::keywords::keywords_of;
use mtg_engine::{Event, replay};
use mtg_testkit::{PlayOptions, play_game_with, split_second_fuzz_setup};

use crate::common::seeds::{for_each_seed, fuzz_games};

/// What the games showed.
#[derive(Default)]
struct Seen {
    casts: u64,
    windows: u64,
    mana_in_windows: u64,
    triggers_in_windows: u64,
    undaunted: u64,
}

/// The first spells of a game that are looked at: each one takes a replay to its cast.
const LOOKED_AT: usize = 8;

#[test]
fn bot_games_with_split_second_and_undaunted_keep_every_invariant_and_replay() {
    let seen = Mutex::new(Seen::default());
    let games = fuzz_games(40);
    for_each_seed(0..games, |seed| {
        for (players, options) in [
            (2, PlayOptions::default()),
            (
                3,
                PlayOptions {
                    partial_payments: true,
                    ..PlayOptions::default()
                },
            ),
            (
                4,
                PlayOptions {
                    partial_payments: true,
                    symbol_choices: true,
                    ..PlayOptions::default()
                },
            ),
        ] {
            let setup = split_second_fuzz_setup(players, seed);
            let outcome =
                play_game_with(setup, seed, options).unwrap_or_else(|failure| panic!("{failure}"));
            let setup = split_second_fuzz_setup(players, seed);
            let replayed = replay(&setup, &outcome.events).unwrap();
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let casts: Vec<usize> = outcome
                .events
                .iter()
                .enumerate()
                .filter(|(_, event)| matches!(event, Event::SpellCast { .. }))
                .map(|(at, _)| at)
                .collect();
            let mut mine = Seen {
                casts: casts.len() as u64,
                ..Seen::default()
            };
            for at in casts.into_iter().take(LOOKED_AT) {
                let Event::SpellCast { object, .. } = &outcome.events[at] else {
                    continue;
                };
                let state = replay(&setup, &outcome.events[..=at]).unwrap();
                let keywords = keywords_of(&state, *object);
                if keywords.contains(&&Keyword::Undaunted) {
                    mine.undaunted += 1;
                }
                if !keywords.contains(&&Keyword::SplitSecond) {
                    continue;
                }
                // The window runs until the spell resolves or leaves the stack.
                let end = outcome.events[at + 1..]
                    .iter()
                    .position(|event| match event {
                        Event::SpellResolved { object: done } => done == object,
                        Event::ZoneChanged { object: moved, .. } => moved == object,
                        _ => false,
                    })
                    .map_or(outcome.events.len(), |offset| at + 1 + offset);
                mine.windows += 1;
                for event in &outcome.events[at + 1..end] {
                    match event {
                        Event::SpellCast { .. } => panic!("seed {seed}: a spell was cast"),
                        Event::AbilityActivated { .. } => {
                            panic!("seed {seed}: an ability was activated")
                        }
                        Event::ManaAdded { .. } => mine.mana_in_windows += 1,
                        Event::AbilityTriggered(_) => mine.triggers_in_windows += 1,
                        _ => {}
                    }
                }
            }
            let mut seen = seen.lock().unwrap();
            seen.casts += mine.casts;
            seen.windows += mine.windows;
            seen.mana_in_windows += mine.mana_in_windows;
            seen.triggers_in_windows += mine.triggers_in_windows;
            seen.undaunted += mine.undaunted;
        }
    });
    let seen = seen.into_inner().unwrap();
    assert!(seen.casts > 0, "spells were cast");
    assert!(seen.windows > 0, "a spell with split second was cast");
    assert!(
        seen.mana_in_windows > 0,
        "some mana ability was used while one was on the stack"
    );
    assert!(
        seen.triggers_in_windows > 0,
        "some ability triggered while one was on the stack"
    );
    assert!(seen.undaunted > 0, "a spell with undaunted was cast");
}
