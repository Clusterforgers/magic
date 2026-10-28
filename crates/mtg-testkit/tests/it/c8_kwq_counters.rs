//! Goal: Overcharged Amalgam plays whole at tables of two, three and four seats: when it exploits a creature its trigger counters the spell or the triggered ability it was cast in answer to, without an exploit nothing is countered, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{cost, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;

fn c8_kwq_rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// A free creature that gains its controller 3 life as it enters, or a free instant that gains 3 life.
fn c8_kwq_opener(trigger: bool) -> Arc<CardDefinition> {
    if trigger {
        let gain = c8_kwq_rules(
            r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"GainLife":{"amount":3}}]}}"#,
        );
        Arc::new(Arc::unwrap_or_clone(free_creature("Healer", 1, 1)).with_rules(gain))
    } else {
        let gain = c8_kwq_rules(r#"{"Spell":{"effects":[{"GainLife":{"amount":3}}]}}"#);
        Arc::new(
            CardDefinition::new("Tonic")
                .with_type(CardType::Instant)
                .with_mana_cost(cost(0, &[]))
                .with_rules(gain),
        )
    }
}

/// Overcharged Amalgam as the corpus has it, for no mana.
fn c8_kwq_amalgam() -> Arc<CardDefinition> {
    let card = corpus::card("Overcharged Amalgam").unwrap();
    Arc::new(Arc::unwrap_or_clone(card).with_mana_cost(cost(0, &[])))
}

/// Passes until seat 0 has priority over the opener's spell or trigger.
fn c8_kwq_until_answerable(driver: &mut Driver, trigger: bool) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        let triggered = driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::AbilityTriggered(_)));
        if matches!(decision.kind, DecisionKind::Priority(_))
            && decision.player == pid(0)
            && !driver.state().stack().is_empty()
            && (triggered || !trigger)
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    panic!("the opener never waited on the stack");
}

/// Answers until the stack is empty in the first main phase: the exploit with `exploit`, sacrificing Fodder.
fn c8_kwq_settle(driver: &mut Driver, exploit: bool) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(exploit)))?;
            }
            DecisionKind::ChooseCards(_) if exploit => {
                let fodder = driver.id_of("Fodder")?;
                driver.answer(Answer::ChooseCards(CardsChoice(vec![fodder])))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the game did not settle");
}

#[rstest]
fn c8_kwq_an_exploit_counters_the_spell_or_ability_below(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] trigger: bool,
    #[values(true, false)] exploit: bool,
) -> Result<(), ScriptError> {
    let opener = c8_kwq_opener(trigger);
    let name = opener.name.clone();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(free_creature("Fodder", 1, 1))])
        .hand(0, vec![opener, c8_kwq_amalgam()]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast(&name)?;
    c8_kwq_until_answerable(&mut driver, trigger)?;
    driver.cast("Overcharged Amalgam")?;
    c8_kwq_settle(&mut driver, exploit)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(life, if exploit { 20 } else { 23 });
    let exploited = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::Exploited { .. }))
        .count();
    assert_eq!(exploited, usize::from(exploit));
    for seat in 1..seats {
        assert_eq!(driver.state().player(pid(seat)).unwrap().life(), 20);
    }
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
