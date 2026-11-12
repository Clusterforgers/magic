//! Goal: an activated ability whose cost says "X can't be 0" announces X from 1 up, takes X = 1 without asking when that is all that can be paid, and is not put on the stack when no X of at least 1 can be paid, as an X counted in mana or in counters removed; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

const MANA_X: &str = r#"{"Activated":{"cost":[{"Mana":"{X}"},{"XAtLeast":1},"Tap"],"effects":[{"GainLife":{"amount":"X"}}]}}"#;

/// A 1/1 with the ability `json`.
fn keeper(json: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(json).unwrap();
    Arc::new(
        CardDefinition::new("Life Keeper")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(1, &[]))
            .with_power_toughness(1, 1)
            .with_rules(rule),
    )
}

#[rstest]
#[case::three_forests(3, Some((1, 3)))]
#[case::one_forest(1, None)]
fn x_that_cannot_be_0_is_announced_from_1(
    #[case] forests: usize,
    #[case] asked: Option<(u32, u32)>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut battlefield = vec![ready(keeper(MANA_X))];
    battlefield.extend((0..forests).map(|_| ready(forest())));
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .battlefield(0, battlefield)
            .at(1, 0, Step::Main1),
        |scenario, seat| scenario.library(seat, vec![forest(); 5]),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Life Keeper")?;
    match &driver.decision()?.kind {
        DecisionKind::ChooseNumber(options) => {
            assert_eq!(Some((options.min, options.max.unwrap())), asked);
        }
        DecisionKind::PayMana(_) => assert_eq!(asked, None, "X = 1 is taken without asking"),
        other => panic!("{}", other.name()),
    }
    Ok(())
}

#[rstest]
fn x_that_cannot_be_0_is_not_announced_without_mana(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .battlefield(0, vec![ready(keeper(MANA_X))])
            .at(1, 0, Step::Main1),
        |scenario, seat| scenario.library(seat, vec![forest(); 5]),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(
        driver.offered_abilities("Life Keeper")?,
        Vec::<usize>::new()
    );
    let _ = driver.begin_activation("Life Keeper");
    assert!(
        driver.state().stack().is_empty(),
        "X = 0 cannot be announced"
    );
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life);
    Ok(())
}
