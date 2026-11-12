//! Goal: "equip abilities you activate that target this creature cost {2} less" changes the cost of an activation by its announced target: an equip that targets the creature pays less, one that targets another creature pays the full cost, the equip is offered while some target makes it affordable, and "abilities that target a Merfolk cost {2} more" never hides an activation that targets something else; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{bear, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An Equipment with Equip {2}.
fn blade() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Blade")
            .with_type(CardType::Artifact)
            .with_rules(Ability::Keyword(Keyword::Equip(Cost::mana(
                "{2}".parse().unwrap(),
            )))),
    )
}

/// A 2/2 with "Equip abilities you activate that target this creature cost {2} `change`."
fn champion(change: &str) -> Arc<CardDefinition> {
    let rule = format!(
        r#"{{"CostModifier":{{"applies_to":[{{"ControlledBy":"You"}}],"change":"{change}","mana":"{{2}}","activating":"Equip","targeting":["This"]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Champion")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(serde_json::from_str::<Ability>(&rule).unwrap()),
    )
}

fn table(seats: usize, change: &str, lands: usize) -> Scenario {
    let mut mine = vec![ready(blade()), ready(champion(change)), ready(bear())];
    mine.extend((0..lands).map(|_| ready(forest())));
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, mine);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| forest()));
    }
    scenario
}

/// Begins the equip, aims it at `name` and returns the mana the payment asks for.
fn equip_cost(driver: &mut Driver, name: &str) -> Result<String, ScriptError> {
    driver.begin_activation("Blade")?;
    let aim = driver.target_named(name)?;
    driver.choose_targets(&[aim])?;
    Ok(driver.pay_options()?.cost.to_string())
}

#[rstest]
#[case::the_creature_pays_less("Reduce", "Champion", "{0}")]
#[case::another_creature_pays_in_full("Reduce", "Grizzly Bears", "{2}")]
#[case::an_increase_for_the_creature("Increase", "Champion", "{4}")]
#[case::no_increase_for_another("Increase", "Grizzly Bears", "{2}")]
fn the_target_announced_sets_the_cost(
    #[values(2, 3, 4)] seats: usize,
    #[case] change: &str,
    #[case] aim: &str,
    #[case] cost: &str,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, change, 4))?;
    assert_eq!(equip_cost(&mut driver, aim)?, cost);
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
#[case::a_reduction_offers_it_with_no_mana("Reduce", 0, true)]
#[case::an_increase_does_not_hide_it("Increase", 2, true)]
#[case::without_mana_for_another_target("Increase", 0, false)]
fn the_equip_is_offered_at_its_cheapest_target(
    #[case] change: &str,
    #[case] lands: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(table(2, change, lands))?;
    assert_eq!(!driver.offered_abilities("Blade")?.is_empty(), offered);
    Ok(())
}
