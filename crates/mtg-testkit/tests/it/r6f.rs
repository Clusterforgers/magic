//! Goal: the engine pieces of round 6 builder f play at 2 to 4 seats: a shield that redirects the next N damage sends that much of each event to the recipient it fixed, keeps the rest with the first recipient and is used up across events, redirects nothing to a recipient that is gone, and a flash enchantment cast off sorcery timing (in answer to an ability, in the end step) is sacrificed at the next cleanup step while one cast in a main phase on an empty stack stays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::Answer;
use mtg_engine::decision::{DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use common::{names, pid};

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

fn with_rules(card: CardDefinition, abilities: &[&str]) -> Arc<CardDefinition> {
    abilities
        .iter()
        .fold(card, |card, json| card.with_rules(rules(json)))
        .into()
}

/// A free sorcery that deals `amount` damage to target creature.
fn bolt(name: &str, amount: u32) -> Arc<CardDefinition> {
    let spell = format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":{amount}}}}}]}}}}"#
    );
    with_rules(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[])),
        &[&spell],
    )
}

/// Answers every decision passively, paying mana automatically, until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            driver.answer(Answer::PayMana(PayChoice::Auto))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn damage(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .damage()
}

/// "{0}: The next `most` damage that would be dealt to this creature this turn is dealt to target creature you control instead."
fn en_kor(most: u32) -> Arc<CardDefinition> {
    let ability = format!(
        r#"{{"Activated":{{"cost":"{{0}}","targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}}}}],"effects":[{{"Shield":{{"when":{{"Damage":{{"to":"This"}}}},"by":{{"RedirectUpTo":{{"most":{most},"to":{{"Target":0}}}}}},"duration":"UntilEndOfTurn"}}}}]}}}}"#
    );
    with_rules(
        CardDefinition::new("Kor")
            .with_type(CardType::Creature)
            .with_power_toughness(0, 9),
        &[&ability],
    )
}

/// Seat 0 controls the redirecting creature and an Ally and holds two bolts of these amounts.
fn table(seats: usize, most: u32, first: u32, second: u32) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [ready(en_kor(most)), ready(free_creature("Ally", 0, 9))])
            .hand(0, [bolt("First", first), bolt("Second", second)]),
    )
}

fn shield_on_ally(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.begin_activation("Kor")?;
    let ally = driver.target_named("Ally")?;
    driver.choose_targets(&[ally])?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    settle(driver)
}

#[rstest]
fn the_next_damage_goes_to_the_fixed_recipient_and_the_rest_stays(
    #[values(2, 3, 4)] seats: usize,
    #[values((1, 1, 2), (1, 3, 2), (2, 1, 3), (3, 2, 2))] amounts: (u32, u32, u32),
) -> Result<(), ScriptError> {
    let (most, first, second) = amounts;
    let mut driver = table(seats, most, first, second)?;
    shield_on_ally(&mut driver)?;
    let kor = driver.target_named("Kor")?;
    driver.cast_at("First", &[kor])?;
    settle(&mut driver)?;
    let moved = most.min(first);
    assert_eq!(damage(&driver, "Ally"), moved);
    assert_eq!(damage(&driver, "Kor"), first - moved);
    driver.cast_at("Second", &[kor])?;
    settle(&mut driver)?;
    let rest = (most - moved).min(second);
    assert_eq!(
        damage(&driver, "Ally"),
        moved + rest,
        "what is left carries over"
    );
    assert_eq!(damage(&driver, "Kor"), first - moved + second - rest);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 20);
    Ok(())
}

#[rstest]
fn a_recipient_that_left_takes_nothing_and_the_damage_stays(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, 1, 9, 2)?;
    shield_on_ally(&mut driver)?;
    let ally = driver.target_named("Ally")?;
    driver.cast_at("First", &[ally])?;
    settle(&mut driver)?;
    let kor = driver.target_named("Kor")?;
    driver.cast_at("Second", &[kor])?;
    settle(&mut driver)?;
    assert_eq!(damage(&driver, "Kor"), 2);
    Ok(())
}

/// When the flash enchantment is cast in turn 1.
#[derive(Debug, Clone, Copy)]
enum When {
    /// In the first main phase with an empty stack: a sorcery could be cast.
    MainPhase,
    /// In the first main phase in answer to an ability on the stack.
    InAnswer,
    /// In the end step.
    EndStep,
}

#[rstest]
fn a_flash_enchantment_cast_off_sorcery_timing_is_sacrificed_at_the_next_cleanup(
    #[values(2, 3, 4)] seats: usize,
    #[values(When::MainPhase, When::InAnswer, When::EndStep)] when: When,
) -> Result<(), ScriptError> {
    let climb = with_rules(
        CardDefinition::new("Climb")
            .with_type(CardType::Enchantment)
            .with_mana_cost(cost(0, &[])),
        &[r#"{"Keyword":"FlashSacrifice"}"#],
    );
    let gadget = with_rules(
        CardDefinition::new("Gadget").with_type(CardType::Artifact),
        &[r#"{"Activated":{"cost":"{0}","effects":[{"GainLife":{"amount":1}}]}}"#],
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [ready(gadget)])
            .hand(0, [climb]),
    )?;
    match when {
        When::MainPhase => {}
        When::InAnswer => {
            driver.begin_activation("Gadget")?;
            if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
        }
        When::EndStep => driver.pass_until(Step::End)?,
    }
    driver.cast("Climb")?;
    settle(&mut driver)?;
    let climb = driver.id_of("Climb")?;
    assert_eq!(
        driver.state().object(climb).unwrap().zone(),
        Zone::Battlefield
    );
    driver.idle_until(2, Step::Upkeep)?;
    let sacrificed = !matches!(when, When::MainPhase);
    let in_zone = |zone| names(driver.state(), zone).contains(&"Climb".to_owned());
    assert_eq!(in_zone(Zone::Graveyard(pid(0))), sacrificed, "{when:?}");
    assert_eq!(in_zone(Zone::Battlefield), !sacrificed, "{when:?}");
    Ok(())
}
