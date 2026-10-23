//! Goal: spectacle is an alternative cost from the hand that is offered only in a turn in which an opponent of the caster lost life (combat damage, a loss or a payment), the spell remembers it was cast that way for "if its spectacle cost was paid", the record ends with the turn, and afterlife and horsemanship cards play beside it; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{choose_way, lands, pay, ways};
use crate::common::{names, on_battlefield, pid};
use mtg_cards::CardDefinition;
use mtg_engine::PresetPermanent;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, mountain, swamp};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn card(name: &str) -> Arc<CardDefinition> {
    corpus::card(name).unwrap()
}

/// Seat 0 holds `hand`, the `lands` and a Bear at the declare attackers step of turn 1; every other seat holds two Forests.
fn table(seats: usize, hand: &[&str], lands: Vec<PresetPermanent>) -> Scenario {
    let mut permanents = vec![ready(card("Grizzly Bears"))];
    permanents.extend(lands);
    (1..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::DeclareAttackers)
            .hand_named(0, hand.iter().copied())
            .battlefield(0, permanents),
        |scenario, seat| scenario.hand(seat, [forest(), forest()]),
    )
}

/// Answers whatever is asked passively until the stack is empty and nothing waits.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Whether beginning the cast of Skewer the Critics asks for the spectacle cost among its ways.
fn offers_spectacle(driver: &mut Driver) -> Result<bool, ScriptError> {
    driver.begin_cast("Skewer the Critics")?;
    Ok(
        matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_))
            && ways(driver)?.iter().any(|way| way.starts_with("Spectacle")),
    )
}

fn mana(red: usize, black: usize) -> Vec<PresetPermanent> {
    lands(red, mountain)
        .into_iter()
        .chain(lands(black, swamp))
        .collect()
}

#[rstest]
#[case::an_opponent_was_dealt_combat_damage(true, true)]
#[case::nobody_lost_life(false, false)]
fn spectacle_is_offered_after_an_opponent_lost_life(
    #[case] attack: bool,
    #[case] offered: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &["Skewer the Critics"], mana(3, 0)))?;
    if attack {
        driver.attack(&[("Grizzly Bears", 1)])?;
        driver.idle_until(1, Step::Main2)?;
    } else {
        driver.attack(&[])?;
        driver.idle_until(1, Step::Main2)?;
    }
    assert_eq!(offers_spectacle(&mut driver)?, offered);
    Ok(())
}

#[rstest]
fn the_record_of_lost_life_ends_with_the_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &["Skewer the Critics"], mana(3, 0)))?;
    driver.attack(&[("Grizzly Bears", 1)])?;
    driver.idle_until(1, Step::Main2)?;
    driver.idle_until(1 + u32::try_from(seats).unwrap(), Step::Main1)?;
    assert!(!offers_spectacle(&mut driver)?);
    Ok(())
}

#[rstest]
#[case::cast_for_spectacle(true, 1)]
#[case::cast_for_its_mana_cost(false, 2)]
fn rafter_demon_makes_each_opponent_discard_only_when_its_spectacle_cost_was_paid(
    #[case] attack: bool,
    #[case] left_in_hand: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &["Rafter Demon"], mana(2, 3)))?;
    if attack {
        driver.attack(&[("Grizzly Bears", 1)])?;
    } else {
        driver.attack(&[])?;
    }
    driver.idle_until(1, Step::Main2)?;
    driver.begin_cast("Rafter Demon")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(&mut driver, "Spectacle")?;
    }
    pay(&mut driver)?;
    settle(&mut driver)?;
    assert!(on_battlefield(driver.state(), "Rafter Demon").is_some());
    for seat in 1..seats {
        assert_eq!(
            driver.state().zone(Zone::Hand(pid(seat))).len(),
            left_in_hand,
            "seat {seat}"
        );
    }
    Ok(())
}

#[rstest]
fn a_choice_of_way_names_the_spectacle_cost(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &["Skewer the Critics"], mana(3, 0)))?;
    driver.attack(&[("Grizzly Bears", 1)])?;
    driver.idle_until(1, Step::Main2)?;
    driver.begin_cast("Skewer the Critics")?;
    let labels = ways(&driver)?;
    assert!(
        labels.iter().any(|way| way == "Spectacle {R}"),
        "{labels:?}"
    );
    choose_way(&mut driver, "Spectacle")?;
    driver.choose_targets(&[driver.target_player(1)])?;
    pay(&mut driver)?;
    settle(&mut driver)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 20 - 2 - 3);
    Ok(())
}

#[rstest]
fn afterlife_makes_its_spirits_when_the_creature_dies(
    #[values(2, 3, 4)] seats: usize,
    #[values(("Ministrant of Obligation", 2), ("Syndicate Messenger", 1), ("Debtors' Transport", 2))]
    (name, spirits): (&str, usize),
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, ["Skewer the Critics"])
        .battlefield(0, mana(3, 0))
        .battlefield(1, [ready(card(name))]);
    for seat in 2..seats {
        scenario = scenario.hand(seat, [forest()]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Skewer the Critics")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(&mut driver, "Cast for")?;
    }
    let target = driver.target_named(name)?;
    driver.choose_targets(&[target])?;
    pay(&mut driver)?;
    settle(&mut driver)?;
    let on_table = names(driver.state(), Zone::Battlefield);
    assert!(!on_table.contains(&name.to_owned()), "{name} died");
    assert_eq!(
        on_table.iter().filter(|found| *found == "Spirit").count(),
        spirits
    );
    Ok(())
}
