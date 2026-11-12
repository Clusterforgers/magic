//! Goal: "the next creature spell you cast this turn can't be countered" (`NextSpell` with `Uncounterable`) keeps the first matching spell cast after it from being countered and only that one: a spell of another kind cast in between neither spends it nor is protected, a second creature spell can be countered, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::copying::{on_the_stack, replays, run};
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn card(name: &str, kind: CardType, rules: Option<&str>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(kind)
        .with_mana_cost(ManaCost::parse("{0}").unwrap());
    if kind == CardType::Creature {
        card = card.with_power_toughness(1, 1);
    }
    if let Some(rules) = rules {
        card = card.with_rules(serde_json::from_str(rules).unwrap());
    }
    Arc::new(card)
}

const INSIST: &str = r#"{"Spell":{"effects":[{"Rule":{"duration":"UntilEndOfTurn","rule":{"NextSpell":{"benefit":"Uncounterable","what":[{"Type":"Creature"}]}}}}]}}"#;
const NEGATOR: &str = r#"{"Activated":{"cost":[],"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#;

/// Seat 0 casts the named spells in order, each resolving before the next, and counters the last with the negator.
fn cast_and_counter(driver: &mut Driver, spells: &[&str]) -> Result<(), ScriptError> {
    let (last, first) = spells.split_last().unwrap();
    for name in first {
        driver.cast(name)?;
        run(driver, 2)?;
    }
    driver.cast(last)?;
    let target = on_the_stack(driver, last);
    driver.begin_activation("Negator")?;
    driver.choose_targets(&[target])?;
    run(driver, 2)
}

#[rstest]
#[case::the_next_creature_resolves(&["Insist", "Bear"], &["Bear"])]
#[case::a_sorcery_in_between_is_countered(&["Insist", "Ritual"], &[])]
#[case::the_creature_after_the_sorcery_resolves(&["Insist", "Ritual", "Bear"], &["Bear"])]
#[case::the_second_creature_is_countered(&["Insist", "Bear", "Cub"], &["Bear"])]
#[case::without_the_rule(&["Bear"], &[])]
fn only_the_next_matching_spell_cannot_be_countered(
    #[case] spells: &[&str],
    #[case] on_battlefield: &[&str],
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .hand(
                0,
                [
                    card("Insist", CardType::Sorcery, Some(INSIST)),
                    card("Ritual", CardType::Sorcery, None),
                    card("Bear", CardType::Creature, None),
                    card("Cub", CardType::Creature, None),
                ],
            )
            .battlefield(
                0,
                vec![ready(card("Negator", CardType::Artifact, Some(NEGATOR)))],
            )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    cast_and_counter(&mut driver, spells)?;
    let creatures: Vec<String> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.card().is_type(CardType::Creature))
        .map(|object| object.card().name.clone())
        .collect();
    assert_eq!(creatures, on_battlefield);
    replays(&driver, &setup);
    Ok(())
}
