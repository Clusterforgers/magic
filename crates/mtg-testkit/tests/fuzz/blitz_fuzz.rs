//! Goal: whole bot games at 2 to 4 seats with the creatures that have blitz keep every invariant, and across the seeds each part of the mechanic occurs: a creature is cast for its blitz cost, one that is still in play at the end step is sacrificed, one that died draws its controller a card, and none cast that way is still in play when the turn is cleaned up.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::CastWay;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::Event;
use mtg_engine::events::ZoneCause;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::{PlayOptions, blitz_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

/// The creatures cast for their blitz cost that are in play, with their casters.
type Blitzed = Vec<(ObjectId, PlayerId)>;

/// The creature `object` left play: it is no longer tracked and, when it died or was sacrificed, its caster is owed a card.
fn left(blitzed: &mut Blitzed, owed: &mut Vec<PlayerId>, object: ObjectId) {
    if let Some(at) = blitzed.iter().position(|(o, _)| *o == object) {
        let (_, player) = blitzed.remove(at);
        owed.push(player);
    }
}

#[test]
fn bot_games_with_blitz_keep_every_invariant_and_use_each_part_of_it() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(12);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                blitz_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            let mut blitzed: Blitzed = Vec::new();
            // The casters of the creatures that left play and whose draw is still to come.
            let mut owed: Vec<PlayerId> = Vec::new();
            for event in &outcome.events {
                match event {
                    Event::SpellCast {
                        player,
                        object,
                        choices,
                    } if choices.way == Some(CastWay::Blitz) => {
                        *seen.entry("blitz cast").or_default() += 1;
                        blitzed.push((*object, *player));
                    }
                    Event::ZoneChanged {
                        object,
                        from: Zone::Battlefield,
                        cause: ZoneCause::Sacrificed,
                        ..
                    } if blitzed.iter().any(|(o, _)| o == object) => {
                        *seen.entry("blitz sacrifice").or_default() += 1;
                        left(&mut blitzed, &mut owed, *object);
                    }
                    Event::CreatureDied { object, .. }
                        if blitzed.iter().any(|(o, _)| o == object) =>
                    {
                        *seen.entry("blitz death").or_default() += 1;
                        left(&mut blitzed, &mut owed, *object);
                    }
                    Event::ZoneChanged { object, from, .. }
                        if *from == Zone::Battlefield
                            && blitzed.iter().any(|(o, _)| o == object) =>
                    {
                        blitzed.retain(|(o, _)| o != object);
                    }
                    Event::CardDrawn { player, .. } if owed.contains(player) => {
                        owed.retain(|owed| owed != player);
                        *seen.entry("blitz draw").or_default() += 1;
                    }
                    Event::StepBegan {
                        step: Step::Cleanup,
                    } => {
                        assert_eq!(
                            blitzed,
                            [],
                            "a blitzed creature is still in play at cleanup (seed {seed}, {players} seats)"
                        );
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for part in ["blitz cast", "blitz sacrifice", "blitz death", "blitz draw"] {
        assert!(
            seen.get(part).copied().unwrap_or(0) > 0,
            "no creature went through {part}: {seen:?}"
        );
    }
}
