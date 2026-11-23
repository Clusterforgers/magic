//! Goal: landwalk of each basic land type among the lands its controller controls (`Pred::R117SharesBasicLandType`, Magnigoth Treefolk, CR 702.14c) plays at tables of two to four seats: the creature can't be blocked exactly when the defending player controls a land with a basic land type one of its controller's lands has, a nonbasic type such as Desert counting for nothing, and the walk follows the lands as they are now.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, free_creature, island, mountain, plains, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;

/// A nonbasic land with the subtype Desert.
fn desert() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Desert")
            .with_type(CardType::Land)
            .with_subtype("Desert"),
    )
}

fn land(name: &str) -> Arc<CardDefinition> {
    match name {
        "Plains" => plains(),
        "Island" => island(),
        "Swamp" => swamp(),
        "Mountain" => mountain(),
        "Forest" => forest(),
        _ => desert(),
    }
}

/// Seat 0 controls Magnigoth Treefolk and `mine`; seat 1 controls a Wall and `theirs`.
fn table(seats: usize, mine: &[&str], theirs: &[&str]) -> Result<Driver, ScriptError> {
    let mut own = vec![ready(corpus::card("Magnigoth Treefolk").unwrap())];
    own.extend(mine.iter().map(|name| ready(land(name))));
    let mut other = vec![ready(free_creature("Wall", 0, 8))];
    other.extend(theirs.iter().map(|name| ready(land(name))));
    Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, own)
            .battlefield(1, other),
    )
}

/// Whether the Wall may block the Treefolk attacking seat 1; not asked at all, it may not.
fn blockable(driver: &mut Driver) -> Result<bool, ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Magnigoth Treefolk", 1)])?;
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::DeclareBlockers(_) if decision.player == pid(1) => {
                return Ok(driver.block(&[("Wall", "Magnigoth Treefolk")]).is_ok());
            }
            // With no creature able to block, the defending player is not asked.
            _ if driver.state().turn().step() > Step::DeclareBlockers => return Ok(false),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("seat 1 was not asked for blockers");
}

#[rstest]
#[case::shared_forest(&["Forest", "Island"], &["Forest"], false)]
#[case::no_shared_type(&["Forest", "Island"], &["Swamp", "Mountain"], true)]
#[case::no_lands_of_mine(&[], &["Plains"], true)]
#[case::desert_is_no_basic_type(&["Desert"], &["Desert"], true)]
#[case::all_five(&["Plains", "Island", "Swamp", "Mountain", "Forest"], &["Mountain"], false)]
fn the_walk_follows_the_shared_basic_land_types(
    #[values(2, 3, 4)] seats: usize,
    #[case] mine: &[&str],
    #[case] theirs: &[&str],
    #[case] can_block: bool,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, mine, theirs)?;
    assert_eq!(blockable(&mut driver)?, can_block);
    Ok(())
}
