//! Goal: whole bot games at two to four seats with convoke and delve cards (the corpus cards, hybrid, Phyrexian and {X} costs beside the keywords) finish with every invariant held, the bots paying in part and by hand with creatures tapped and cards exiled, and the log of each game replays to its final state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Mutex;

use mtg_engine::{Event, replay};
use mtg_testkit::{PlayOptions, help_fuzz_setup, play_game_with};

use crate::common::seeds::for_each_seed;

/// What the games showed: payments that tapped creatures, payments that exiled cards, and the spells cast in all.
#[derive(Default)]
struct Seen {
    convoked: u64,
    delved: u64,
    casts: u64,
}

#[test]
fn bot_games_with_convoke_and_delve_keep_every_invariant_and_replay() {
    let seen = Mutex::new(Seen::default());
    let games = std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40);
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
            let outcome = play_game_with(help_fuzz_setup(players, seed), seed, options)
                .unwrap_or_else(|failure| panic!("{failure}"));
            let replayed = replay(&help_fuzz_setup(players, seed), &outcome.events).unwrap();
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let mut seen = seen.lock().unwrap();
            let mut last_cast_at = None;
            for (at, event) in outcome.events.iter().enumerate() {
                match event {
                    Event::SpellCast { .. } => {
                        seen.casts += 1;
                        last_cast_at = Some(at);
                    }
                    Event::PermanentTapped { object } => {
                        // A creature tapped right after a cast and not for mana is a convoke.
                        let by_mana = outcome.events[at + 1..].iter().take(1).any(|e| {
                            matches!(e, Event::ManaAdded { source: Some(s), .. } if s == object)
                        });
                        if last_cast_at.is_some_and(|cast| cast + 8 > at)
                            && !by_mana
                            && replayed.object(*object).is_some_and(|o| {
                                o.card().types.contains(&mtg_core::card::CardType::Creature)
                            })
                        {
                            seen.convoked += 1;
                        }
                    }
                    Event::ZoneChanged {
                        to: mtg_engine::zones::Zone::Exile,
                        from: mtg_engine::zones::Zone::Graveyard(_),
                        ..
                    } => seen.delved += 1,
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    assert!(seen.casts > 0, "spells were cast");
    assert!(seen.convoked > 0, "some creature was tapped for convoke");
    assert!(seen.delved > 0, "some card was exiled for delve");
}
