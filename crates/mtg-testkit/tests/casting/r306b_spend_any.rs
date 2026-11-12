//! Goal: a card cast during a resolution with "mana of any type can be spent to cast that spell" is paid with mana of
//! any colour, and without that rider a cost its caster's mana cannot pay is not offered; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, island};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{pid, spell};

/// A red sorcery costing {R}{R}: "You gain 5 life."
fn red_gift() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Red Gift")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[Color::Red, Color::Red]))
            .with_rules(
                serde_json::from_str(r#"{"Spell":{"effects":[{"GainLife":{"amount":5}}]}}"#)
                    .unwrap(),
            ),
    )
}

/// "You may cast target sorcery card from a graveyard[, and mana of any type can be spent to cast that spell]."
fn recall(any_type: bool) -> Arc<CardDefinition> {
    let spend = if any_type {
        r#","spend_as":"AnyType""#
    } else {
        ""
    };
    spell(
        "Recall",
        CardType::Sorcery,
        r#"[{"what":{"Object":[{"Type":"Sorcery"},{"In":"Graveyard"}]}}]"#,
        &format!(r#"[{{"CastFromZone":{{"what":{{"Target":0}}{spend}}}}}]"#),
    )
}

fn table(seats: usize, any_type: bool) -> Scenario {
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .graveyard(0, [red_gift()])
        .hand(0, vec![recall(any_type)])
        .battlefield(0, vec![ready(island()), ready(island())])
}

/// Casts Recall at the red card and answers until the stack is empty; how many times a cast was offered.
fn recall_the_gift(driver: &mut Driver) -> Result<usize, ScriptError> {
    let gift = driver.state().zone(Zone::Graveyard(pid(0)))[0];
    let aim = mtg_engine::choose::Target::Object(driver.state().object(gift).unwrap().reference());
    driver.cast_at("Recall", &[aim])?;
    let mut offered = 0;
    for _ in 0..60 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(offered);
        }
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::YesNo(_) => {
                offered += 1;
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn blue_mana_pays_a_red_cost_when_any_type_may_be_spent(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, true);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(recall_the_gift(&mut driver)?, 1);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 25);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn without_the_rider_the_red_cost_is_not_offered(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, false))?;
    assert_eq!(recall_the_gift(&mut driver)?, 0);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 20);
    assert_eq!(driver.state().zone(Zone::Graveyard(pid(0))).len(), 2);
    Ok(())
}
