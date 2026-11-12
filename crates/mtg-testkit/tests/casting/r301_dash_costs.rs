//! Goal: "Dash costs you pay cost {2} less" (Warbringer) takes {2} off a spell cast for its dash cost and nothing off the same spell cast for its mana cost, and only while Warbringer is on the battlefield, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{choose_way, lands};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::DecisionKind;
use mtg_testkit::cards::{island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A creature with mana cost {3}{R} and dash {2}{R}.
fn raider() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Raider")
            .with_type(CardType::Creature)
            .with_mana_cost(ManaCost::parse("{3}{R}").unwrap())
            .with_power_toughness(2, 2)
            .with_rules(Ability::Keyword(Keyword::Dash(Cost::mana(
                "{2}{R}".parse().unwrap(),
            )))),
    )
}

/// Seat 0 with the Raider in hand, five Mountains and Warbringer on the battlefield or not.
fn table(seats: usize, warbringer: bool) -> Scenario {
    let mut permanents = lands(5, mountain);
    if warbringer {
        permanents.push(ready(card("Warbringer").unwrap()));
    }
    let mut scenario = Scenario::new(seats)
        .hand(0, [raider()])
        .battlefield(0, permanents);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    scenario
}

/// The mana value of the cost asked to cast the Raider the way `label` names.
fn asked(driver: &mut Driver, label: &str) -> Result<u32, ScriptError> {
    driver.begin_cast("Raider")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(driver, label)?;
    }
    Ok(driver.pay_options()?.cost.mana_value())
}

#[rstest]
#[case::two_seats_dash(2, true, "Dash", 1)]
#[case::three_seats_printed(3, true, "Cast for", 4)]
#[case::four_seats_dash(4, true, "Dash", 1)]
#[case::without_warbringer(2, false, "Dash", 3)]
fn warbringer_takes_two_off_the_dash_cost(
    #[case] seats: usize,
    #[case] warbringer: bool,
    #[case] way: &str,
    #[case] value: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, warbringer))?;
    assert_eq!(asked(&mut driver, way)?, value);
    Ok(())
}
