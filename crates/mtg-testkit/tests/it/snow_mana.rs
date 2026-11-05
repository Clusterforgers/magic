//! Goal: an ability that costs {S} is offered only when a snow source can pay it, the engine taps the snow land and not a plain one, two {S} need two snow sources, and the mana of the snow land counts as snow mana spent.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::mana::ManaCostX;
use mtg_cards::refs::{Ref, Targetable, ZoneKind};
use mtg_cards::spend::SpendOn;
use mtg_core::card::{CardType, Supertype};
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;
use mtg_engine::Event;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_testkit::cards::{damage, one_target};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A creature whose ability costs `price` and deals 1 damage to a target.
fn shade(price: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Shade")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(Ability::Activated {
                cost: Cost(vec![CostItem::Mana(ManaCostX::new(
                    0,
                    ManaCost::parse(price).unwrap(),
                ))]),
                timing: Timing::Instant,
                condition: None,
                limit: None,
                by: mtg_cards::ability::Activators::Controller,
                from: ZoneKind::Battlefield,
                targets: vec![one_target(Targetable::AnyTarget)],
                effects: vec![damage(Ref::Target(0), 1)],
            }),
    )
}

fn mountain(snow: bool) -> Arc<CardDefinition> {
    let land = CardDefinition::basic_land(Some(Color::Red));
    Arc::new(if snow {
        land.with_supertype(Supertype::Snow)
    } else {
        land
    })
}

fn offered(driver: &Driver) -> bool {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    !options.abilities.is_empty()
}

fn table(price: &str, snow: usize, plain: usize) -> Result<Driver, ScriptError> {
    let lands: Vec<_> = (0..plain)
        .map(|_| ready(mountain(false)))
        .chain((0..snow).map(|_| ready(mountain(true))))
        .collect();
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [vec![ready(shade(price))], lands].concat()),
    )?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    Ok(driver)
}

#[rstest]
#[case::one_snow_land("{S}", 1, 0, true)]
#[case::plain_land_only("{S}", 0, 1, false)]
#[case::no_land("{S}", 0, 0, false)]
#[case::snow_among_plain("{S}", 1, 2, true)]
#[case::two_symbols_one_snow("{S}{S}", 1, 2, false)]
#[case::two_symbols_two_snow("{S}{S}", 2, 1, true)]
#[case::generic_with_snow("{1}{S}", 1, 1, true)]
#[case::generic_snow_all_plain("{1}{S}", 0, 2, false)]
fn a_snow_cost_is_offered_only_with_enough_snow_sources(
    #[case] price: &str,
    #[case] snow: usize,
    #[case] plain: usize,
    #[case] expected: bool,
) -> Result<(), ScriptError> {
    let driver = table(price, snow, plain)?;
    assert_eq!(offered(&driver), expected);
    Ok(())
}

#[test]
fn the_engine_pays_with_the_snow_land_and_counts_the_snow_mana() -> Result<(), ScriptError> {
    let mut driver = table("{S}", 1, 2)?;
    driver.begin_activation("Shade")?;
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe])?;
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let tapped: Vec<ObjectId> = events
        .iter()
        .filter_map(|e| match e {
            Event::PermanentTapped { object } => Some(*object),
            _ => None,
        })
        .collect();
    assert_eq!(tapped.len(), 1);
    let land = driver.state().object(tapped[0]).unwrap();
    assert!(land.card().supertypes.contains(&Supertype::Snow));
    let snow_spent: usize = events
        .iter()
        .map(|e| match e {
            Event::ManaSpent { restricted, .. } => restricted
                .iter()
                .filter(|unit| unit.only.contains(&SpendOn::Snow))
                .count(),
            _ => 0,
        })
        .sum();
    assert_eq!(snow_spent, 1);
    Ok(())
}
