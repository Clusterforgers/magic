//! Goal: an airbended card (exiled, its owner may cast it for {2} rather than its mana cost, CR 118.9) is offered to its owner alone, only while it stays exiled and only when {2} can be paid, is cast for {2} whatever its printed cost, never for the printed cost, and ends on the battlefield like any spell.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, creature, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, names, pid};

/// A free instant: "Airbend target creature." (Exile it; its owner may cast it for {2} rather than its mana cost.)
fn gust() -> Arc<CardDefinition> {
    let json = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Exile":{"what":{"Target":0},"as":"exiled"}},{"Rule":{"rule":{"MayCastRather":{"only":{"Bound":"exiled"},"cost":"{2}"}},"duration":"Forever"}}]}}"#;
    Arc::new(
        CardDefinition::new("Gust")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(serde_json::from_str::<Ability>(json).unwrap()),
    )
}

fn castable(driver: &Driver, name: &str) -> bool {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return false;
    };
    options
        .castable
        .iter()
        .any(|id| name_of(driver.state(), *id) == name)
}

/// Seat 0 casts Gust on the Bear of seat 1 (which has `lands` Forests); the Bear costs {3}{G}.
fn airbended(lands: usize) -> Result<Driver, ScriptError> {
    let bear = creature("Bear", 3, &[Color::Green], 2, 2);
    let mut driver = Driver::from_scenario(Scenario::new(2).hand(0, [gust()]).battlefield(
        1,
        std::iter::once(ready(bear)).chain((0..lands).map(|_| ready(forest()))),
    ))?;
    let target = driver.target_named("Bear")?;
    driver.cast_at("Gust", &[target])?;
    while !driver.state().stack().is_empty() {
        driver.answer_passively()?;
    }
    assert_eq!(names(driver.state(), Zone::Exile), ["Bear"]);
    driver.idle_until(2, Step::Main1)?;
    Ok(driver)
}

#[rstest]
#[case::two_lands_pay_it(2, true)]
#[case::more_lands_than_the_cost_need(4, true)]
#[case::one_land_is_short(1, false)]
#[case::no_land(0, false)]
fn the_owner_may_cast_it_exactly_when_two_mana_can_be_paid(
    #[case] lands: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = airbended(lands)?;
    assert_eq!(driver.state().turn().active(), pid(1));
    assert_eq!(castable(&driver, "Bear"), offered);
    Ok(())
}

#[test]
fn casting_it_costs_two_and_puts_it_on_the_battlefield_under_its_owner() -> Result<(), ScriptError>
{
    let mut driver = airbended(2)?;
    driver.cast("Bear")?;
    while !driver.state().stack().is_empty() {
        driver.answer_passively()?;
    }
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Bear".to_owned()));
    assert!(
        names(driver.state(), Zone::Exile).is_empty(),
        "the card left exile"
    );
    for _ in 0..2 {
        assert!(driver.state().zone(Zone::Battlefield).iter().any(|id| {
            let tapped = driver
                .state()
                .object(*id)
                .and_then(|o| o.permanent())
                .is_some_and(mtg_engine::object::PermanentState::tapped);
            tapped && name_of(driver.state(), *id) == "Forest"
        }));
    }
    Ok(())
}

#[test]
fn the_player_who_airbended_it_may_not_cast_it() -> Result<(), ScriptError> {
    let bear = creature("Bear", 0, &[], 2, 2);
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [gust()])
            .battlefield(0, [ready(forest()), ready(forest())])
            .battlefield(1, [ready(bear)]),
    )?;
    let target = driver.target_named("Bear")?;
    driver.cast_at("Gust", &[target])?;
    while !driver.state().stack().is_empty() {
        driver.answer_passively()?;
    }
    assert!(!castable(&driver, "Bear"), "its owner is the other seat");
    Ok(())
}
