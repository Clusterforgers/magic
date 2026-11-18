//! Goal: a seat with the opening hand rule (a Delves boss) keeps seven cards holding two to four lands without being
//! asked to mulligan, whatever its deck's land share and the seed; its library keeps the rest, and the same seed deals
//! the same hand again.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::Engine;
use mtg_engine::decision::{Answer, DecisionKind, MulliganChoice};
use mtg_engine::format::Commander;
use mtg_engine::opening_lands::OpeningLands;
use mtg_engine::setup::GameSetup;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest};
use mtg_testkit::{Driver, ScriptError};
use proptest::prelude::*;

const DECK: usize = 99;

/// A Commander table of `heroes` hero seats and a boss seat last whose 99-card deck holds `lands` lands at random
/// places, the boss with the opening hand rule.
fn table(lands: usize, heroes: usize, seed: u64) -> Result<Driver, ScriptError> {
    let deck = |lands: usize| -> Vec<Arc<CardDefinition>> {
        (0..DECK)
            .map(|at| {
                if (at * 7 + 3) % DECK < lands {
                    forest()
                } else {
                    creature("Filler", 1, &[], 1, 1)
                }
            })
            .collect()
    };
    let decks = (0..=heroes)
        .map(|seat| deck(if seat == heroes { lands } else { 38 }))
        .collect();
    let mut setup = GameSetup::new(Arc::new(Commander), seed, decks);
    setup.validate_decks = false;
    if let Some(boss) = setup.players.last_mut() {
        boss.opening_lands = Some(OpeningLands::BOSS);
    }
    Driver::new(Engine::new(setup)?)
}

/// Keeps every hero's hand; panics if the boss is asked.
fn keep_heroes(driver: &mut Driver, boss: PlayerId) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Mulligan(_)) {
        assert_ne!(
            driver.decision()?.player,
            boss,
            "the boss takes no mulligan"
        );
        driver.answer(Answer::Mulligan(MulliganChoice::Keep))?;
    }
    Ok(())
}

fn hand(driver: &Driver, boss: PlayerId) -> Vec<ObjectId> {
    driver.state().zone(Zone::Hand(boss)).to_vec()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn the_boss_keeps_two_to_four_lands_and_replays(
        lands in 4usize..60,
        heroes in 1usize..4,
        seed in any::<u64>(),
    ) {
        let boss = PlayerId::from_index(heroes);
        let mut driver = table(lands, heroes, seed).unwrap();
        keep_heroes(&mut driver, boss).unwrap();
        let held = hand(&driver, boss);
        let state = driver.state();
        let in_hand = held
            .iter()
            .filter(|id| state.object(**id).unwrap().base_types().contains(CardType::Land))
            .count();
        prop_assert_eq!(held.len(), 7);
        prop_assert!((2..=4).contains(&in_hand), "{} lands", in_hand);
        prop_assert_eq!(state.zone(Zone::Library(boss)).len(), DECK - 7);
        prop_assert_eq!(state.player(boss).unwrap().mulligans(), 0);
        let mut again = table(lands, heroes, seed).unwrap();
        keep_heroes(&mut again, boss).unwrap();
        prop_assert_eq!(hand(&again, boss), held);
        prop_assert_eq!(
            again.state().zone(Zone::Library(boss)),
            driver.state().zone(Zone::Library(boss))
        );
    }
}
