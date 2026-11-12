//! Goal: "The next creature spell you cast this turn costs {2} less to cast." (`GameRule::NextSpell`) changes the cost of the next matching spell of its controller and ends with that cast: a spell of another kind leaves it, the second matching spell pays in full, and the opponent's casts neither get it nor use it up.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{lands, resolve};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// A free sorcery that starts `rule` (the JSON of a `NextSpell`) until the end of the turn.
fn prelude(rule: &str) -> Arc<CardDefinition> {
    let spell: Ability = serde_json::from_str(&format!(
        r#"{{"Spell":{{"effects":[{{"Rule":{{"rule":{rule},"duration":"UntilEndOfTurn"}}}}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Prelude")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(spell),
    )
}

fn creature_less() -> &'static str {
    r#"{"NextSpell":{"what":[{"Type":"Creature"}],"benefit":{"Cost":{"change":"Reduce","mana":"{2}"}}}}"#
}

/// The cost as the engine writes it.
fn written(text: &str) -> String {
    ManaCost::parse(text).unwrap().to_string()
}

/// Seat 0 holds Prelude and `hand`, with eight Forests; it casts Prelude and then each spell of `before` (resolving
/// each), and starts casting `name`, whose total cost is returned.
fn cost_after(
    rule: &str,
    hand: &[&str],
    before: &[&str],
    name: &str,
) -> Result<String, ScriptError> {
    let mut cards = vec![prelude(rule)];
    for name in hand {
        cards.push(corpus::card(name).unwrap());
    }
    let scenario = Scenario::new(2)
        .battlefield(0, lands(8, forest).into_iter().collect::<Vec<_>>())
        .hand(0, cards);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Prelude")?;
    resolve(&mut driver)?;
    for spell in before {
        driver.cast(spell)?;
        resolve(&mut driver)?;
    }
    driver.begin_cast(name)?;
    Ok(driver.pay_options()?.cost.to_string())
}

#[rstest]
#[case::the_next_creature(&[], "{G}")]
#[case::the_second_creature_pays_in_full(&["Grizzly Bears"], "{1}{G}")]
#[case::an_artifact_before_leaves_it(&["Sol Ring"], "{G}")]
fn the_next_creature_spell_costs_less(
    #[case] before: &[&str],
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let hand = ["Grizzly Bears", "Grizzly Bears", "Sol Ring"];
    assert_eq!(
        cost_after(creature_less(), &hand, before, "Grizzly Bears")?,
        written(expected)
    );
    Ok(())
}

#[rstest]
#[case::a_cost_increase(r#"{"NextSpell":{"benefit":{"Cost":{"change":"Increase","mana":"{1}"}}}}"#, &[], "{2}{G}")]
#[case::any_spell_is_the_next(r#"{"NextSpell":{"benefit":{"Cost":{"change":"Reduce","mana":"{1}"}}}}"#, &[], "{G}")]
#[case::any_spell_is_used_by_an_artifact(r#"{"NextSpell":{"benefit":{"Cost":{"change":"Reduce","mana":"{1}"}}}}"#, &["Sol Ring"], "{1}{G}")]
fn any_next_spell(
    #[case] rule: &str,
    #[case] before: &[&str],
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let hand = ["Grizzly Bears", "Sol Ring"];
    assert_eq!(
        cost_after(rule, &hand, before, "Grizzly Bears")?,
        written(expected)
    );
    Ok(())
}
