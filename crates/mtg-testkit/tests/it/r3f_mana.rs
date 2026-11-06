//! Goal: a replacement of the mana of a tap, whether a permanent holds it or a resolved spell does for a turn, decides what a land pays (CR 106.6, 614.1): a spell is offered exactly when the replaced mana can pay for it and then is cast with that mana, a tap for mana never uses the stack, and the replacement ends with its turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, land};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, names};

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

const RULE: &str = r#"{"ManaProduced":{"what":[{"Type":"Land"}],"who":"EachPlayer","change":{"OnlyType":"Black"}}}"#;

/// An enchantment: "Lands you tap for mana produce {B} instead."
fn contamination() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Contamination")
            .with_type(CardType::Enchantment)
            .with_rules(rules(&format!(r#"{{"Rule":{{"rule":{RULE}}}}}"#))),
    )
}

/// A free instant: "Until end of turn, lands tapped for mana produce {B} instead."
fn dusk() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Dusk")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules(&format!(
                r#"{{"Spell":{{"effects":[{{"Rule":{{"rule":{RULE},"duration":"UntilEndOfTurn"}}}}]}}}}"#
            ))),
    )
}

/// A creature that costs {B}.
fn shade() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Shade")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[Color::Black]))
            .with_power_toughness(1, 1),
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

#[rstest]
#[case::none(false, false)]
#[case::a_permanent_holds_it(true, false)]
#[case::a_spell_made_it_for_the_turn(false, true)]
fn a_forest_pays_for_a_black_spell_only_while_a_replacement_makes_it_black(
    #[case] permanent: bool,
    #[case] spell: bool,
) -> Result<(), ScriptError> {
    let mut hand = vec![shade()];
    if spell {
        hand.push(dusk());
    }
    let mut battlefield = vec![ready(forest())];
    if permanent {
        battlefield.push(ready(contamination()));
    }
    let mut driver =
        Driver::from_scenario(Scenario::new(2).hand(0, hand).battlefield(0, battlefield))?;
    assert_eq!(castable(&driver, "Shade"), permanent);
    if spell {
        driver.cast("Dusk")?;
        while !driver.state().stack().is_empty() {
            driver.answer_passively()?;
        }
    }
    let replaced = permanent || spell;
    assert_eq!(castable(&driver, "Shade"), replaced);
    if replaced {
        driver.cast("Shade")?;
        while !driver.state().stack().is_empty() {
            driver.answer_passively()?;
        }
        assert_eq!(
            names(driver.state(), Zone::Battlefield).len(),
            2 + usize::from(permanent)
        );
        assert!(names(driver.state(), Zone::Battlefield).contains(&"Shade".to_owned()));
    }
    Ok(())
}

#[test]
fn a_tap_for_replaced_mana_adds_it_without_the_stack() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [ready(land(Color::Green)), ready(contamination())]),
    )?;
    let log = driver.tap_for_mana("Forest")?;
    assert!(driver.state().stack().is_empty());
    let added: Vec<ManaType> = log
        .iter()
        .filter_map(|event| match event {
            mtg_engine::Event::ManaAdded { mana, .. } => Some(*mana),
            _ => None,
        })
        .collect();
    assert_eq!(added, [ManaType::Black]);
    Ok(())
}

#[test]
fn the_replacement_of_a_spell_ends_with_the_turn() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [dusk(), shade()])
            .battlefield(0, [ready(forest())]),
    )?;
    driver.cast("Dusk")?;
    while !driver.state().stack().is_empty() {
        driver.answer_passively()?;
    }
    assert!(castable(&driver, "Shade"));
    driver.idle_until(2, mtg_engine::turn::Step::Main1)?;
    driver.idle_until(3, mtg_engine::turn::Step::Main1)?;
    assert!(!castable(&driver, "Shade"));
    Ok(())
}
