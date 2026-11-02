//! Goal: It That Betrays puts the nontoken permanents its opponents sacrifice onto its controller's battlefield, whether the sacrifice comes from its own annihilator or from another player's effect, and leaves tokens alone, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, edict, free_creature, swamp};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// How many permanents the seat controls.
fn permanents(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().controller() == pid(seat))
        .count()
}

/// Whether a permanent called `name` is on the battlefield under the seat's control.
fn holds(driver: &Driver, seat: usize, name: &str) -> bool {
    driver.id_of(name).is_ok_and(|id| {
        let object = driver.state().object(id).unwrap();
        object.zone() == Zone::Battlefield && object.controller() == pid(seat)
    })
}

/// Plays on until the stack is empty at a priority decision, sacrificing the first permanents offered.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::OrderTriggers(options) => {
                let order: Vec<usize> = (0..options.triggers.len()).collect();
                driver.order_triggers(&order)?;
            }
            DecisionKind::ChooseCards(options) => {
                let picks = options
                    .from
                    .iter()
                    .copied()
                    .take(options.min as usize)
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(picks)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            other => return Err(ScriptError::Stuck(format!("a decision: {}", other.name()))),
        }
    }
    Err(ScriptError::Stuck("the stack never settled".into()))
}

fn betrays() -> Arc<CardDefinition> {
    corpus::card("It That Betrays").unwrap()
}

/// A sorcery for `{B}`: its controller makes a Soldier token and sacrifices a token.
fn token_sacrifice() -> Arc<CardDefinition> {
    let rules = r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}}}},{"Sacrifice":{"who":"You","filter":["Token"],"count":1}}]}}"#;
    Arc::new(
        CardDefinition::new("Spend Token")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[Color::Black]))
            .with_rules(serde_json::from_str::<Ability>(rules).unwrap()),
    )
}

#[rstest]
#[case::two_of_four(4, 2)]
#[case::fewer_than_the_number(1, 1)]
fn annihilator_sacrifices_end_up_under_the_betrayers_control(
    #[values(2, 3, 4)] seats: usize,
    #[case] held: usize,
    #[case] taken: usize,
) -> Result<(), ScriptError> {
    let defender = seats - 1;
    let mut table = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(betrays())]);
    for seat in 1..seats {
        let pawns = if seat == defender { held } else { 3 };
        table = table.battlefield(
            seat,
            (0..pawns)
                .map(|i| ready(free_creature(&format!("Pawn{seat}{i}"), 1, 1)))
                .collect::<Vec<_>>(),
        );
    }
    let mut driver = Driver::from_scenario(table)?;
    driver.attack(&[("It That Betrays", defender)])?;
    settle(&mut driver)?;
    assert_eq!(permanents(&driver, defender), held - taken);
    assert_eq!(permanents(&driver, 0), 1 + taken);
    for seat in 1..defender {
        assert_eq!(permanents(&driver, seat), 3, "seat {seat}");
    }
    Ok(())
}

#[rstest]
fn a_sacrifice_by_another_effect_is_taken_too(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Seat 1 is made to sacrifice by an edict from the Betrays's own controller.
    let mut table = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [edict("Edict")])
        .battlefield(0, [ready(betrays()), ready(swamp()), ready(swamp())])
        .battlefield(1, [ready(free_creature("Ogre", 3, 3))]);
    for seat in 2..seats {
        table = table.battlefield(
            seat,
            [ready(free_creature(&format!("Bystander{seat}"), 1, 1))],
        );
    }
    let mut driver = Driver::from_scenario(table)?;
    driver.cast_at("Edict", &[driver.target_player(1)])?;
    settle(&mut driver)?;
    assert!(holds(&driver, 0, "Ogre"), "the Ogre changed sides");
    assert_eq!(permanents(&driver, 1), 0);
    for seat in 2..seats {
        assert_eq!(permanents(&driver, seat), 1, "seat {seat}");
    }
    Ok(())
}

#[rstest]
fn a_sacrificed_token_is_not_taken(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut table = Scenario::new(seats)
        .at(2, 1, Step::Main1)
        .battlefield(0, [ready(betrays())])
        .battlefield(1, [ready(swamp())])
        .hand(1, [token_sacrifice()]);
    for seat in 2..seats {
        table = table.battlefield(
            seat,
            [ready(free_creature(&format!("Bystander{seat}"), 1, 1))],
        );
    }
    let mut driver = Driver::from_scenario(table)?;
    driver.cast("Spend Token")?;
    settle(&mut driver)?;
    assert_eq!(permanents(&driver, 0), 1);
    assert!(driver.id_of("Soldier").is_err(), "the token is gone");
    Ok(())
}
