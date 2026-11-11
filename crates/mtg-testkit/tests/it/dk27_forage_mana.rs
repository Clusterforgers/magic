//! Goal: a mana ability that forages as part of its cost (Thornvault Forager) pays for a spell at 2 and 3 seats by
//! sacrificing a Food while there is one, else by exiling three graveyard cards, and the spell is not offered when the
//! payer can do neither; "whenever you sacrifice one or more Foods" (Camellia) triggers once for each batch of Foods
//! sacrificed, whether one or two go together, and not for another permanent.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, creature, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

const FORAGER: &str = r#"{"Activated":{"cost":["Tap","Forage"],"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":2}}}}]}}"#;
const CAMELLIA: &str = r#"{"Triggered":{"when":{"Batch":{"Sacrificed":{"what":[{"Subtype":"Food"}],"by":"You"}}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Squirrel","colors":["Green"],"types":["Creature"],"subtypes":["Squirrel"],"power":1,"toughness":1}}}}]}}"#;

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

fn food(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_subtype("Food"),
    )
}

fn with(card: &Arc<CardDefinition>, json: &str) -> Arc<CardDefinition> {
    Arc::new((**card).clone().with_rules(rules(json)))
}

fn dead(count: usize) -> Vec<Arc<CardDefinition>> {
    (0..count)
        .map(|at| creature(&format!("Dead {at}"), 1, &[], 1, 1))
        .collect()
}

/// Seat 0 has the Forager, `foods` Foods, `cards` graveyard cards and a blue-red creature to cast with the mana.
fn table(seats: usize, foods: usize, cards: usize, camellia: bool) -> Scenario {
    let forager = with(&free_creature("Forager", 1, 2), FORAGER);
    let pair = creature("Pair", 0, &[Color::Blue, Color::Red], 2, 2);
    let mut permanents = vec![ready(forager)];
    permanents.extend((0..foods).map(|_| ready(food("Snack"))));
    if camellia {
        permanents.push(ready(with(&free_creature("Camellia", 3, 3), CAMELLIA)));
    }
    Scenario::new(seats)
        .hand(0, [pair])
        .battlefield(0, permanents)
        .graveyard(0, dead(cards))
}

/// Passes and answers passively until the stack and the waiting triggers are empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let idle =
            driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty();
        let priority = matches!(
            driver.decision()?.kind,
            mtg_engine::decision::DecisionKind::Priority(_)
        );
        if idle && priority {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn squirrels(driver: &Driver) -> usize {
    names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| *name == "Squirrel")
        .count()
}

#[rstest]
#[case::a_food(1, 0, 1, 0)]
#[case::a_food_before_the_graveyard(1, 5, 1, 0)]
#[case::three_cards(0, 4, 0, 3)]
fn the_forage_of_the_mana_ability_pays_for_the_spell(
    #[case] foods: usize,
    #[case] cards: usize,
    #[case] sacrificed: usize,
    #[case] exiled: usize,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, foods, cards, true))?;
    driver.cast("Pair")?;
    assert!(!driver.state().stack().is_empty(), "the Pair is cast");
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    assert_eq!(
        graveyard.iter().filter(|name| *name == "Snack").count(),
        sacrificed
    );
    assert_eq!(names(driver.state(), Zone::Exile).len(), exiled);
    settle(&mut driver)?;
    assert_eq!(squirrels(&driver), sacrificed, "Camellia sees the Food");
    Ok(())
}

#[rstest]
#[case::nothing(0, 0)]
#[case::two_cards(0, 2)]
fn without_a_food_or_three_cards_the_spell_is_not_offered(
    #[case] foods: usize,
    #[case] cards: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, foods, cards, false))?;
    assert!(driver.cast("Pair").is_err(), "nothing pays for the Pair");
    Ok(())
}

/// "Sacrifice N Foods" as a sorcery of its own, or of the artifacts when `artifacts`.
fn feast(count: u32, artifacts: bool) -> Arc<CardDefinition> {
    let filter = if artifacts {
        r#"[{"Type":"Artifact"}]"#
    } else {
        r#"[{"Subtype":"Food"}]"#
    };
    let spell = format!(
        r#"{{"Spell":{{"effects":[{{"Sacrifice":{{"filter":{filter},"count":{count}}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Feast")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules(&spell)),
    )
}

#[rstest]
#[case::one_food(1, 1, false, 1, 1)]
#[case::two_foods_together(2, 2, false, 2, 1)]
#[case::an_artifact_that_is_no_food(0, 1, true, 0, 0)]
fn one_or_more_foods_sacrificed_together_make_one_squirrel(
    #[case] foods: usize,
    #[case] count: u32,
    #[case] artifacts: bool,
    #[case] gone: usize,
    #[case] made: usize,
) -> Result<(), ScriptError> {
    let camellia = with(&free_creature("Camellia", 3, 3), CAMELLIA);
    let relic = Arc::new(CardDefinition::new("Relic").with_type(CardType::Artifact));
    let mut permanents = vec![ready(camellia)];
    permanents.extend((0..foods).map(|_| ready(food("Snack"))));
    if artifacts {
        permanents.push(ready(relic));
    }
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [feast(count, artifacts)])
            .battlefield(0, permanents),
    )?;
    driver.cast("Feast")?;
    settle(&mut driver)?;
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    assert_eq!(
        graveyard.iter().filter(|name| *name == "Snack").count(),
        gone
    );
    assert_eq!(squirrels(&driver), made);
    Ok(())
}
