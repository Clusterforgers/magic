//! Goal: the targets asked after a cost has picked what it moves (a sacrifice, an exile, a return) mark those objects as `paid`, whichever of them was picked, so a client can warn that such a target is gone before the ability resolves; a pick that leaves the object where it is marks nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::name_of;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The names of the objects the pending target question marks as taken by the cost.
fn paid(driver: &Driver) -> Vec<String> {
    let DecisionKind::ChooseTargets(options) = &driver.decision().unwrap().kind else {
        panic!("targets were expected, not {:?}", driver.decision());
    };
    options
        .paid
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect()
}

#[rstest]
#[case::sacrifice_wolf("Goblin Bombardment", "Wolf", &["Wolf"])]
#[case::sacrifice_ogre("Goblin Bombardment", "Ogre", &["Ogre"])]
#[case::tap_wolf("Field Surgeon", "Wolf", &[])]
fn the_targets_mark_what_the_cost_moves(
    #[case] card: &str,
    #[case] pick: &str,
    #[case] expected: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [
            ready(corpus::card(card).unwrap()),
            ready(free_creature("Wolf", 2, 2)),
            ready(free_creature("Ogre", 3, 3)),
        ],
    ))?;
    driver.begin_activation(card)?;
    let id = driver.id_of(pick)?;
    driver.answer(Answer::ChooseCards(CardsChoice(vec![id])))?;
    assert_eq!(paid(&driver), expected);
    Ok(())
}
