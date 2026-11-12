//! Goal: "The first creature spell you cast each turn costs {2} less to cast." and "The first artifact spell ..." reduce the first spell of their kind the caster casts in a turn and no later one: a spell of another kind cast before leaves the reduction for the first of its own, the spell being cast does not count itself, and the opponent's spells of the kind do not use it up.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{lands, resolve};
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An enchantment whose only ability is a reduction of `mana` for the first spell of `kind` its controller casts each
/// turn, written as the compiler reads "The first creature spell you cast each turn costs {2} less to cast."
fn first_of(kind: &str, tally: &str, mana: &str) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"CostModifier":{{"applies_to":["Spell",{{"Type":"{kind}"}},{{"ControlledBy":"You"}}],"change":"Reduce","mana":"{mana}","condition":{{"Compare":{{"left":{{"Tallied":{{"who":"You","what":"{tally}"}}}},"op":"Eq","right":0}}}}}}}}"#
    );
    Arc::new(
        CardDefinition::new("First Discount")
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str(&json).unwrap()),
    )
}

/// The cost as the engine writes it.
fn cost(text: &str) -> String {
    ManaCost::parse(text).unwrap().to_string()
}

/// Seat 0 controls the reduction and eight Forests and holds `hand`; it casts and resolves each spell of `before` in
/// turn, then starts casting `name`, whose total cost is returned.
fn cost_after(
    discount: Arc<CardDefinition>,
    hand: &[&str],
    before: &[&str],
    name: &str,
) -> Result<String, ScriptError> {
    let mut permanents = lands(8, forest);
    permanents.push(ready(discount));
    let scenario = Scenario::new(2)
        .battlefield(0, permanents)
        .hand_named(0, hand.to_vec());
    let mut driver = Driver::from_scenario(scenario)?;
    for spell in before {
        driver.cast(spell)?;
        resolve(&mut driver)?;
    }
    driver.begin_cast(name)?;
    Ok(driver.pay_options()?.cost.to_string())
}

#[rstest]
#[case::the_first_creature(&[], "{G}")]
#[case::the_second_creature(&["Grizzly Bears"], "{1}{G}")]
#[case::after_an_artifact(&["Sol Ring"], "{G}")]
fn the_first_creature_spell_each_turn_costs_less(
    #[case] before: &[&str],
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let discount = first_of("Creature", "CreatureSpellsCast", "{2}");
    let hand = ["Grizzly Bears", "Grizzly Bears", "Sol Ring"];
    assert_eq!(
        cost_after(discount, &hand, before, "Grizzly Bears")?,
        cost(expected)
    );
    Ok(())
}

#[rstest]
#[case::the_first_artifact(&[], "")]
#[case::the_second_artifact(&["Sol Ring"], "{1}")]
#[case::after_a_creature(&["Grizzly Bears"], "")]
fn the_first_artifact_spell_each_turn_costs_less(
    #[case] before: &[&str],
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let discount = first_of("Artifact", "ArtifactSpellsCast", "{1}");
    let hand = ["Sol Ring", "Sol Ring", "Grizzly Bears"];
    assert_eq!(
        cost_after(discount, &hand, before, "Sol Ring")?,
        cost(expected)
    );
    Ok(())
}
