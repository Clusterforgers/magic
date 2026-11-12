//! Goal: "Kicker {A} and/or {B}" plays whole at tables of two, three and four seats: each kicker is its own optional cost, paid alone or together or not at all, and an ability that asks "if it was kicked with its {A} kicker" runs for that kicker only (Stormscape Battlemage on entering, Wastescape Battlemage as it is cast), against the player the controller targets and nobody else.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, pid};
use mtg_engine::choose::CostsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, island, plains, swamp, wastes};
use mtg_testkit::corpus::card;
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 holds `name` and controls these lands; every other seat controls a Bear named after its seat, and seat 1 a Relic.
fn table(
    seats: usize,
    name: &str,
    lands: Vec<mtg_engine::PresetPermanent>,
) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand(0, [card(name).unwrap()])
        .battlefield(0, lands);
    for seat in 1..seats {
        let mut permanents = vec![ready(free_creature(&format!("Bear {seat}"), 2, 2))];
        if seat == 1 {
            permanents.push(ready(artifact_with("Relic 1", Vec::new())));
        }
        scenario = scenario.battlefield(seat, permanents);
    }
    Driver::from_scenario(scenario)
}

/// `count` lands made by `land`, ready.
fn lands(
    count: usize,
    land: fn() -> std::sync::Arc<mtg_cards::CardDefinition>,
) -> Vec<mtg_engine::PresetPermanent> {
    (0..count).map(|_| ready(land())).collect()
}

/// Casts `name`, paying the kickers at these places in the card's list (the optional costs come first), and pays the
/// total with the lands.
fn cast(driver: &mut Driver, name: &str, kicks: &[usize]) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    driver.expect_decision(0, "choose-costs")?;
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(kicks.to_vec())))?;
    Ok(())
}

/// Pays what the cast asks, with targets chosen at the first of `wanted` each ask accepts, until the stack is empty at a
/// priority decision.
fn settle(driver: &mut Driver, wanted: &[&str]) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let mut chosen = false;
                for name in wanted {
                    let target = driver.target_named(name)?;
                    if driver.choose_targets(&[target]).is_ok() {
                        chosen = true;
                        break;
                    }
                }
                assert!(chosen, "none of {wanted:?} is a legal target");
            }
            DecisionKind::OrderTriggers(options) => {
                let order: Vec<usize> = (0..options.triggers.len()).collect();
                driver.order_triggers(&order)?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the stack did not empty");
}

fn has(driver: &Driver, zone: Zone, name: &str) -> bool {
    names(driver.state(), zone)
        .iter()
        .any(|found| found == name)
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
fn stormscape_battlemage_runs_the_ability_of_each_kicker_that_was_paid(
    #[values(2, 3, 4)] seats: usize,
    #[values(&[], &[0], &[1], &[0, 1])] kicks: &[usize],
) -> Result<(), ScriptError> {
    let mut all = lands(3, island);
    all.extend(lands(1, plains));
    all.extend(lands(3, swamp));
    let mut driver = table(seats, "Stormscape Battlemage", all)?;
    cast(&mut driver, "Stormscape Battlemage", kicks)?;
    settle(&mut driver, &["Bear 1"])?;
    assert!(has(&driver, Zone::Battlefield, "Stormscape Battlemage"));
    let life_gain = if kicks.contains(&0) { 3 } else { 0 };
    assert_eq!(life(&driver, 0), 20 + life_gain);
    // The {2}{B} kicker destroys the creature chosen, and only that one.
    let destroyed = kicks.contains(&1);
    assert_eq!(has(&driver, Zone::Battlefield, "Bear 1"), !destroyed);
    for seat in 2..seats {
        assert!(has(&driver, Zone::Battlefield, &format!("Bear {seat}")));
    }
    for seat in 1..seats {
        assert_eq!(life(&driver, seat), 20);
    }
    Ok(())
}

#[rstest]
fn wastescape_battlemage_runs_the_ability_of_each_kicker_that_was_paid_as_it_is_cast(
    #[values(2, 3, 4)] seats: usize,
    #[values(&[], &[0], &[1], &[0, 1])] kicks: &[usize],
) -> Result<(), ScriptError> {
    let mut all = lands(2, wastes);
    all.extend(lands(1, forest));
    all.extend(lands(2, island));
    let mut driver = table(seats, "Wastescape Battlemage", all)?;
    cast(&mut driver, "Wastescape Battlemage", kicks)?;
    settle(&mut driver, &["Relic 1", "Bear 1"])?;
    assert!(has(&driver, Zone::Battlefield, "Wastescape Battlemage"));
    // The {G} kicker exiles the artifact; the {1}{U} kicker returns the Bear to its owner's hand.
    assert_eq!(
        has(&driver, Zone::Battlefield, "Relic 1"),
        !kicks.contains(&0)
    );
    assert_eq!(
        has(&driver, Zone::Battlefield, "Bear 1"),
        !kicks.contains(&1)
    );
    assert_eq!(
        has(&driver, Zone::Hand(pid(1)), "Bear 1"),
        kicks.contains(&1)
    );
    for seat in 2..seats {
        assert!(has(&driver, Zone::Battlefield, &format!("Bear {seat}")));
    }
    Ok(())
}

#[test]
fn both_kickers_are_offered_by_their_own_cost() -> Result<(), ScriptError> {
    let mut all = lands(3, island);
    all.extend(lands(1, plains));
    all.extend(lands(3, swamp));
    let mut driver = table(2, "Stormscape Battlemage", all)?;
    driver.begin_cast("Stormscape Battlemage")?;
    let DecisionKind::ChooseCosts(options) = &driver.decision()?.kind else {
        panic!("the optional costs come first");
    };
    let labels: Vec<&str> = options
        .optional
        .iter()
        .map(|cost| cost.label.as_str())
        .collect();
    assert_eq!(labels, ["Kicker {W}", "Kicker {2}{B}"]);
    Ok(())
}
