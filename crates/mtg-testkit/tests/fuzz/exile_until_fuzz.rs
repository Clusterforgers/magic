//! Goal: bot games with the exile-until and mana-spent cards finish at two, three and four seats, replay exactly from their seed, and really exile cards until a source leaves, return each only after its source has left, sacrifice creatures that were not cast with the mana they ask for and keep those that were.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};

use crate::common::name_of;
use crate::common::seeds::fuzz_games;
use mtg_core::ids::ObjectId;
use mtg_engine::events::ZoneCause;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::{SPENT_CARDS, exile_until_fuzz_setup, play_game};
use rstest::rstest;

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_exile_until_exile_and_return_cards_and_sacrifice_unless_the_mana_was_spent(
    #[case] seats: usize,
) {
    let (mut exiled, mut returned, mut sacrificed, mut kept) = (0, 0, 0, 0);
    for seed in 0..fuzz_games(80) {
        let setup = exile_until_fuzz_setup(seats, seed).unwrap();
        let outcome = play_game(setup.clone(), seed)
            .unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        let names = replay(&setup, &outcome.events).unwrap();
        let spender = |id: ObjectId| {
            SPENT_CARDS
                .iter()
                .any(|(name, _)| *name == name_of(&names, id))
        };
        let mut left: BTreeSet<ObjectId> = BTreeSet::new();
        let mut waiting: BTreeMap<ObjectId, ObjectId> = BTreeMap::new();
        let mut cast: BTreeSet<ObjectId> = BTreeSet::new();
        let mut slain: BTreeSet<ObjectId> = BTreeSet::new();
        for event in &outcome.events {
            if let Event::CreatureDied { object, .. } | Event::ObjectLeftGame { object } = event {
                left.insert(*object);
            }
            if let Event::SpellResolved { object } = event
                && spender(*object)
            {
                cast.insert(*object);
            }
            let Event::ZoneChanged {
                object,
                from,
                to,
                cause,
            } = event
            else {
                continue;
            };
            if *from == Zone::Battlefield {
                left.insert(*object);
            }
            if *to == Zone::Battlefield {
                if *from == Zone::Stack && spender(*object) {
                    cast.insert(*object);
                }
                if let Some(source) = waiting.remove(object) {
                    assert!(
                        left.contains(&source),
                        "seed {seed}: {object:?} returned while {source:?} was still there: {:#?}",
                        outcome
                            .events
                            .iter()
                            .filter(|e| e.touched_objects().contains(object)
                                || e.touched_objects().contains(&source))
                            .collect::<Vec<_>>()
                    );
                    returned += 1;
                }
            }
            if let ZoneCause::ExiledUntil { by } = cause {
                waiting.insert(*object, by.id);
                exiled += 1;
            }
            if *cause == ZoneCause::Sacrificed && cast.contains(object) && slain.insert(*object) {
                sacrificed += 1;
            }
        }
        kept += cast.len() - slain.len();
    }
    assert!(exiled > 0, "a card was exiled until its source left");
    assert!(returned > 0, "a card returned when its source left");
    assert!(
        sacrificed > 0,
        "a creature was sacrificed ({kept} stayed, {exiled} exiled, {returned} returned)"
    );
    assert!(kept > 0, "a creature stayed on the battlefield");
}

#[rstest]
#[case::two(2, 51)]
#[case::four(4, 52)]
fn a_game_with_exile_until_repeats_exactly_from_its_seed(#[case] seats: usize, #[case] seed: u64) {
    let play = || {
        let setup = exile_until_fuzz_setup(seats, seed).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
}
