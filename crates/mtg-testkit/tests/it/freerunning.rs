//! Goal: freerunning is an alternative cost from the hand offered only on a turn in which an Assassin or a commander its caster controlled dealt combat damage to a player; another creature does not open it; the log replays; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Supertype};
use mtg_core::color::Color;
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Commander;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, forest, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{lands, ways};

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// A sorcery for {3}{B}{B} with freerunning {B} that draws a card.
fn run() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Rooftop Run")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(3, &[Color::Black, Color::Black]))
            .with_rules(rules(r#"{"Keyword":{"Freerunning":"{B}"}}"#))
            .with_rules(rules(r#"{"Spell":{"effects":[{"Draw":{}}]}}"#)),
    )
}

/// A 2/2 for {0} with haste and the creature type given.
fn runner(name: &str, subtype: &str, legendary: bool) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_subtype(subtype)
        .with_mana_cost(cost(0, &[]))
        .with_power_toughness(2, 2)
        .with_rules(rules(r#"{"Keyword":"Haste"}"#));
    if legendary {
        card = card.with_supertype(Supertype::Legendary);
    }
    Arc::new(card)
}

/// Whether the cast of Rooftop Run asks for its freerunning cost among its ways.
fn offers_freerunning(driver: &mut Driver) -> Result<bool, ScriptError> {
    driver.begin_cast("Rooftop Run")?;
    Ok(
        matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_))
            && ways(driver)?
                .iter()
                .any(|way| way.starts_with("Freerunning")),
    )
}

#[rstest]
#[case::an_assassin_connected("Assassin", false, true)]
#[case::a_commander_connected("Bear", true, true)]
#[case::another_creature_connected("Bear", false, false)]
fn freerunning_is_offered_after_an_assassin_or_a_commander_dealt_combat_damage(
    #[case] subtype: &str,
    #[case] commander: bool,
    #[case] offered: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let attacker = runner("Runner", subtype, commander);
    let mut scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .at(1, 0, Step::Main1)
        .hand(0, vec![run()])
        .battlefield(0, lands(5, swamp));
    scenario = if commander {
        scenario.commanders(0, [attacker])
    } else {
        scenario.battlefield(0, [ready(attacker)].into_iter().chain(lands(5, swamp)))
    };
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..8).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    if commander {
        driver.cast("Runner")?;
        while !driver.state().stack().is_empty() {
            driver.pass()?;
        }
    }
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("Runner", 1)])?;
    driver.idle_until(1, Step::Main2)?;
    assert_eq!(offers_freerunning(&mut driver)?, offered);
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
