//! Goal: whole bot games at 2 to 4 seats with the cards that have mayhem and the cards that discard keep every invariant, and across the seeds each part of the mechanic occurs: a card is discarded and cast from the graveyard for its mayhem cost, and only ever in the turn its caster discarded it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};

use mtg_cards::value::CastWay;
use mtg_engine::Event;
use mtg_engine::events::ZoneCause;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::{PlayOptions, mayhem_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_mayhem_keep_every_invariant_and_cast_only_what_was_discarded_this_turn() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(16);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                mayhem_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            // The cards each seat discarded in the turn so far.
            let mut discarded: BTreeSet<(usize, mtg_core::ids::ObjectId)> = BTreeSet::new();
            for event in &outcome.events {
                match event {
                    Event::StepBegan {
                        step: Step::Untap, ..
                    } => discarded.clear(),
                    Event::CardDiscarded { player, object } => {
                        discarded.insert((player.index(), *object));
                        *seen.entry("discard").or_default() += 1;
                    }
                    Event::ZoneChanged {
                        object,
                        from: Zone::Hand(player),
                        to: Zone::Graveyard(_),
                        cause: ZoneCause::Discarded,
                    } => {
                        discarded.insert((player.index(), *object));
                    }
                    Event::SpellCast {
                        player,
                        object,
                        choices,
                    } if choices.way == Some(CastWay::Mayhem) => {
                        assert!(
                            discarded.contains(&(player.index(), *object)),
                            "a card was cast for its mayhem cost in a turn its caster did not discard it (seed {seed}, {players} seats)"
                        );
                        *seen.entry("mayhem cast").or_default() += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for part in ["discard", "mayhem cast"] {
        assert!(
            seen.get(part).copied().unwrap_or(0) > 0,
            "no card went through {part}: {seen:?}"
        );
    }
}
