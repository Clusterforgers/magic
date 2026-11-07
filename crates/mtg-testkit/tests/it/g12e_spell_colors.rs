//! Goal: an effect that changes the colour of a spell on the stack plays whole at tables of two, three and four seats:
//! the spell has the colour while it is on the stack, the permanent a creature spell becomes keeps it (CR 400.7a), a
//! card that leaves the stack for a graveyard is a new object with its printed colours, and a change "until end of
//! turn" of a permanent spell ends at the cleanup step.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::{object_of, on_the_stack, settle};
use crate::common::pid;

/// An artifact with "{T}: Target spell or permanent becomes red" for `duration`.
fn painter(duration: &str) -> Arc<CardDefinition> {
    let rule = format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"Continuous":{{"affected":{{"Target":0}},"duration":"{duration}","mods":[{{"SetColors":["Red"]}}]}}}}],"targets":[{{"what":{{"Object":[{{"AnyOf":["Spell",{{"In":"Battlefield"}}]}}]}}}}]}}}}"#
    );
    let rules: Ability = serde_json::from_str(&rule).unwrap();
    Arc::new(
        CardDefinition::new("Painter")
            .with_type(CardType::Artifact)
            .with_rules(rules),
    )
}

/// A free instant that gains its caster 1 life.
fn tonic() -> Arc<CardDefinition> {
    let rules: Ability =
        serde_json::from_str(r#"{"Spell":{"effects":[{"GainLife":{"amount":1}}]}}"#).unwrap();
    Arc::new(
        CardDefinition::new("Tonic")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

/// Seat 0 casts `spell` from hand and, holding priority, paints it red with the Painter.
fn painted(
    seats: usize,
    spell: Arc<CardDefinition>,
    duration: &str,
) -> Result<Driver, ScriptError> {
    let name = spell.name.clone();
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [spell])
            .battlefield(0, [ready(painter(duration))]),
    )?;
    driver.cast(&name)?;
    let target = on_the_stack(&driver, &name);
    driver.begin_activation("Painter")?;
    driver.choose_targets(&[target])?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    Ok(driver)
}

fn red(driver: &Driver, id: mtg_core::ids::ObjectId) -> bool {
    characteristics(driver.state(), id)
        .unwrap()
        .colors
        .contains(Color::Red)
}

#[rstest]
fn a_creature_spell_made_red_becomes_a_red_creature(
    #[values(2, 3, 4)] seats: usize,
    #[values("Forever", "UntilEndOfTurn")] duration: &str,
) -> Result<(), ScriptError> {
    let mut driver = painted(seats, free_creature("Bear", 2, 2), duration)?;
    let bear = object_of(on_the_stack(&driver, "Bear"));
    assert!(!red(&driver, bear), "the ability has not resolved yet");
    settle(&mut driver)?;
    let bear = driver.id_of("Bear")?;
    assert_eq!(
        driver.state().object(bear).unwrap().zone(),
        Zone::Battlefield
    );
    assert!(
        red(&driver, bear),
        "the permanent keeps the colour of the spell"
    );
    let next = driver.state().turn().number() + 1;
    driver.idle_until(next, Step::Upkeep)?;
    let bear = driver.id_of("Bear")?;
    assert_eq!(red(&driver, bear), duration == "Forever");
    Ok(())
}

#[rstest]
fn an_instant_made_red_is_red_on_the_stack_only(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = painted(seats, tonic(), "Forever")?;
    let tonic = object_of(on_the_stack(&driver, "Tonic"));
    // The Painter's ability resolves first; the Tonic waits below it.
    for _ in 0..seats {
        if driver.state().stack().len() == 1 {
            break;
        }
        driver.pass()?;
    }
    assert_eq!(driver.state().stack().len(), 1);
    assert!(red(&driver, tonic), "the spell is red on the stack");
    settle(&mut driver)?;
    let tonic = *driver
        .state()
        .zone(Zone::Graveyard(pid(0)))
        .first()
        .expect("the Tonic is in the graveyard");
    assert!(
        !red(&driver, tonic),
        "the card in the graveyard is a new object"
    );
    Ok(())
}
