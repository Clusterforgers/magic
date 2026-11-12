//! Goal: whole bot games at 2 to 4 seats with creatures that have mutate keep every invariant, including the bookkeeping of the merged permanents (their cards are no objects of the game), and across the seeds each part of the mechanic occurs: a creature is cast for its mutate cost, goes over a creature and under one, a mutating spell whose target is gone resolves as a creature spell, and a merged permanent that leaves the battlefield puts each of its cards into the new zone as an object of its own.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::CastWay;
use mtg_core::ids::ObjectId;
use mtg_engine::Event;
use mtg_engine::zones::Zone;
use mtg_testkit::{PlayOptions, mutate_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

/// The object that left the battlefield in `event`, if the event moved one away.
fn departed(event: &Event) -> Option<ObjectId> {
    match event {
        Event::ZoneChanged {
            object,
            from: Zone::Battlefield,
            ..
        }
        | Event::CreatureDied { object, .. } => Some(*object),
        _ => None,
    }
}

#[test]
fn bot_games_with_mutate_keep_every_invariant_and_use_each_part_of_it() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(12);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                mutate_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            // The mutating spells on the stack, and the cards merged into each permanent.
            let mut casting: Vec<ObjectId> = Vec::new();
            let mut merged: BTreeMap<ObjectId, Vec<ObjectId>> = BTreeMap::new();
            let mut checked = false;
            for (at, event) in outcome.events.iter().enumerate() {
                match event {
                    Event::SpellCast {
                        object, choices, ..
                    } if choices.way == Some(CastWay::Mutate) => {
                        *seen.entry("mutate cast").or_default() += 1;
                        casting.push(*object);
                    }
                    Event::Mutated {
                        object,
                        spell,
                        over,
                        ..
                    } => {
                        casting.retain(|cast| cast != spell);
                        merged.entry(*object).or_default().push(*spell);
                        *seen
                            .entry(if *over { "mutate over" } else { "mutate under" })
                            .or_default() += 1;
                    }
                    Event::SpellResolved { object } if casting.contains(object) => {
                        casting.retain(|cast| cast != object);
                        *seen.entry("mutate target gone").or_default() += 1;
                    }
                    Event::ZoneChanged {
                        object,
                        from: Zone::Stack,
                        ..
                    } => casting.retain(|cast| cast != object),
                    other => {
                        let Some(object) = departed(other) else {
                            continue;
                        };
                        let Some(cards) = merged.remove(&object) else {
                            continue;
                        };
                        *seen.entry("merged permanent left").or_default() += 1;
                        // Replaying costs the whole game so far: the first departure of a game is looked at.
                        if checked {
                            continue;
                        }
                        checked = true;
                        let state = mtg_engine::replay(
                            &mutate_fuzz_setup(players, seed),
                            &outcome.events[..=at],
                        )
                        .unwrap();
                        let zone = state
                            .object(object)
                            .map(mtg_engine::object::GameObject::zone);
                        for card in cards {
                            let Some(part) = state.object(card) else {
                                // A mutate card that left the game with its owner.
                                continue;
                            };
                            assert!(
                                part.merged().is_none() && part.counter() > 0,
                                "{card:?} is not an object of its own (seed {seed}, {players} seats)"
                            );
                            if let (Some(zone), true) = (zone, part.zone() != Zone::Battlefield) {
                                assert_eq!(
                                    part.zone(),
                                    zone,
                                    "{card:?} went elsewhere than its permanent (seed {seed}, {players} seats)"
                                );
                                *seen.entry("merged card separate").or_default() += 1;
                            }
                        }
                    }
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for part in [
        "mutate cast",
        "mutate over",
        "mutate under",
        "mutate target gone",
        "merged permanent left",
        "merged card separate",
    ] {
        assert!(
            seen.get(part).copied().unwrap_or(0) > 0,
            "no creature went through {part}: {seen:?}"
        );
    }
}
