//! Goal: whole bot games at 2 to 4 seats with the creatures that have encore keep every invariant, and across the seeds each part of the mechanic occurs: a card is encored, the copies are paired with distinct opponents, a copy that attacks attacks its own opponent, and the copies that are still in play at the end step are sacrificed, so none is in play when the turn is cleaned up.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::Event;
use mtg_engine::events::ZoneCause;
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::{PlayOptions, encore_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

/// The copies an encore made that are in play, each with the opponent it must attack.
type Encored = Vec<(ObjectId, PlayerId)>;

#[test]
fn bot_games_with_encore_keep_every_invariant_and_use_each_part_of_it() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(12);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                encore_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            let mut encored: Encored = Vec::new();
            for event in &outcome.events {
                match event {
                    Event::RuleCreated { effect } => {
                        let Rule::MustAttackPlayers(pairs) = &effect.rule else {
                            continue;
                        };
                        *seen.entry("encore").or_default() += 1;
                        let mut opponents: Vec<PlayerId> =
                            pairs.iter().map(|(_, player)| *player).collect();
                        opponents.sort();
                        opponents.dedup();
                        assert_eq!(
                            opponents.len(),
                            pairs.len(),
                            "two copies are paired with one opponent (seed {seed}, {players} seats)"
                        );
                        encored.extend(pairs.iter().map(|(object, player)| (object.id, *player)));
                    }
                    Event::AttackersDeclared { attackers, .. } => {
                        for (object, target) in attackers {
                            let Some((_, player)) = encored.iter().find(|(o, _)| o == object)
                            else {
                                continue;
                            };
                            *seen.entry("encore attack").or_default() += 1;
                            assert_eq!(
                                *target,
                                mtg_engine::combat::AttackTarget::Player(*player),
                                "a copy attacks an opponent other than its own (seed {seed}, {players} seats)"
                            );
                        }
                    }
                    Event::ZoneChanged {
                        object,
                        from: Zone::Battlefield,
                        cause,
                        ..
                    } if encored.iter().any(|(o, _)| o == object) => {
                        if *cause == ZoneCause::Sacrificed {
                            *seen.entry("encore sacrifice").or_default() += 1;
                        }
                        encored.retain(|(o, _)| o != object);
                    }
                    Event::CreatureDied { object, .. } => encored.retain(|(o, _)| o != object),
                    Event::StepBegan {
                        step: Step::Cleanup,
                    } => {
                        assert_eq!(
                            encored,
                            [],
                            "an encored copy is still in play at cleanup (seed {seed}, {players} seats)"
                        );
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for part in ["encore", "encore attack", "encore sacrifice"] {
        assert!(
            seen.get(part).copied().unwrap_or(0) > 0,
            "no card went through {part}: {seen:?}"
        );
    }
}
