//! Goal: an Aura that enchants a creature card in a graveyard (Animate Dead) at 2 and 3 seats targets a creature card in
//! any graveyard and nothing on the battlefield, resolves attached to the card, returns it under the caster's control
//! with the Aura attached and its power lowered, and the creature is sacrificed when the Aura leaves the battlefield; with
//! no creature card in a graveyard it cannot be cast.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_engine::{GameState, characteristics};
use mtg_testkit::cards::{cost, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

const ENCHANT: &str = r#"{"Keyword":{"Enchant":[{"Type":"Creature"},{"In":"Graveyard"}]}}"#;
const RETURN: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"Matches":{"subject":"This","selector":[{"In":"Battlefield"}]}},"effects":[{"Move":{"what":"Attached","to":{"Battlefield":{"under":"You"}},"as":"returned"}},{"Attach":{"what":"This","to":{"Bound":"returned"}}},{"DelayedTrigger":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield"}},"effects":[{"Sacrifice":{"who":{"ControllerOf":{"Bound":"returned"}},"filter":[{"Is":{"Bound":"returned"}}]}}]}}]}}"#;
const WEAKER: &str =
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":-1,"toughness":0}}]}}"#;
const SHATTER: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Enchantment"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;

fn rules(json: &str) -> mtg_cards::ability::Ability {
    serde_json::from_str(json).unwrap()
}

fn animate() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Animate")
            .with_type(CardType::Enchantment)
            .with_subtype("Aura")
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules(ENCHANT))
            .with_rules(rules(RETURN))
            .with_rules(rules(WEAKER)),
    )
}

fn shatter() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Shatter")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules(SHATTER)),
    )
}

fn find(state: &GameState, name: &str) -> Target {
    state
        .objects()
        .find(|object| object.card().name == name)
        .map(|object| Target::Object(object.reference()))
        .unwrap()
}

/// Passes and answers passively until the stack and the waiting triggers are empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let idle =
            driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty();
        if idle && matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 holds the Aura and a Shatter; `owner` has a 3/3 "Giant" in their graveyard; seat 1 has a 2/2 on the battlefield.
fn table(seats: usize, owner: usize) -> Scenario {
    Scenario::new(seats)
        .hand(0, [animate(), shatter()])
        .graveyard(owner, [free_creature("Giant", 3, 3)])
        .battlefield(1, [ready(free_creature("Bear", 2, 2))])
}

#[rstest]
fn the_card_returns_under_the_caster_with_the_aura_and_is_sacrificed_with_it(
    #[values(0, 1)] owner: usize,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, owner))?;
    driver.begin_cast("Animate")?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the Aura targets");
    };
    let giant = find(driver.state(), "Giant");
    assert_eq!(
        options.legal,
        vec![giant],
        "only the creature card in a graveyard"
    );
    driver.choose_targets(&[giant])?;
    driver.answer(mtg_engine::decision::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    settle(&mut driver)?;
    let state = driver.state();
    assert!(names(state, Zone::Battlefield).contains(&"Giant".to_owned()));
    let Target::Object(giant) = find(state, "Giant") else {
        unreachable!()
    };
    let aura = find(state, "Animate").object().unwrap().id;
    assert_eq!(state.object(giant.id).unwrap().controller(), pid(0));
    assert_eq!(
        state.host_of(aura),
        Some(giant.id),
        "the Aura is attached to it"
    );
    assert_eq!(characteristics(state, giant.id).unwrap().power, Some(2));
    // The Aura leaves: the creature is sacrificed into its owner's graveyard.
    let target = find(driver.state(), "Animate");
    driver.cast_at("Shatter", &[target])?;
    settle(&mut driver)?;
    let state = driver.state();
    assert!(!names(state, Zone::Battlefield).contains(&"Giant".to_owned()));
    assert!(names(state, Zone::Graveyard(pid(owner))).contains(&"Giant".to_owned()));
    Ok(())
}

#[test]
fn without_a_creature_card_in_a_graveyard_the_aura_is_not_cast() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [animate()])
            .battlefield(1, [ready(free_creature("Bear", 2, 2))]),
    )?;
    assert!(driver.begin_cast("Animate").is_err());
    Ok(())
}
