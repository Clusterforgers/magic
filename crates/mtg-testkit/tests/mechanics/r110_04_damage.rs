//! Goal: damage amounts read from the turn and preventions asked at each event, at tables of two to four seats: Impact
//! Resonance divides the most damage one source dealt to one permanent or player this turn (several sources never
//! add up, none makes it zero and the spell is cast with no target), and Battletide Alchemist's controller chooses at
//! each damage to a player whether to prevent as much of it as they control Clerics, and Magma Pummeler prevents damage
//! while it has a +1/+1 counter, removes as many as it can and deals the number removed to a target; the logs replay to
//! the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::{DivideChoice, Target, YesNoChoice};
use mtg_engine::decision::{DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::{Answer, GameSetup, replay};
use mtg_testkit::cards::{burn, free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn damage_on(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .damage()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// Answers every decision passively until the stack is empty; a yes or no goes to `yes`.
fn settle(driver: &mut Driver, yes: bool) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 holds Impact Resonance and `zaps` Zaps (3 damage each) with enough Mountains; the last seat controls Ogre (4/4)
/// and Giant (5/5).
fn resonance_table(seats: usize, zaps: usize) -> Scenario {
    let mut hand = vec![card("Impact Resonance").unwrap()];
    hand.extend((0..zaps).map(|i| burn(&format!("Zap {i}"), 3)));
    Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(0, (0..zaps + 2).map(|_| ready(mountain())))
        .battlefield(
            seats - 1,
            [
                ready(free_creature("Ogre", 4, 4)),
                ready(free_creature("Giant", 5, 5)),
            ],
        )
}

#[rstest]
fn r110_impact_resonance_divides_the_most_one_source_dealt_to_one(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] zaps: usize,
) -> Result<(), ScriptError> {
    let setup = resonance_table(seats, zaps).setup().unwrap();
    let mut driver = Driver::from_scenario(resonance_table(seats, zaps))?;
    let foe = driver.target_player(seats - 1);
    for zap in 0..zaps {
        driver.cast_at(&format!("Zap {zap}"), &[foe])?;
        settle(&mut driver, true)?;
    }
    driver.begin_cast("Impact Resonance")?;
    if zaps == 0 {
        driver.choose_targets(&[])?;
    } else {
        let chosen: Vec<Target> = ["Ogre", "Giant"]
            .iter()
            .map(|name| driver.target_named(name).unwrap())
            .collect();
        driver.choose_targets(&chosen)?;
        let split = vec![(chosen[0], 2), (chosen[1], 1)];
        driver.answer(Answer::DivideAmount(DivideChoice::Division(split)))?;
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver, true)?;
    let (ogre, giant) = if zaps == 0 { (0, 0) } else { (2, 1) };
    assert_eq!(damage_on(&driver, "Ogre"), ogre);
    assert_eq!(damage_on(&driver, "Giant"), giant);
    replays(&driver, &setup);
    Ok(())
}

fn cleric(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype("Cleric")
            .with_power_toughness(1, 1),
    )
}

/// Seat 0 controls Battletide Alchemist and `clerics` other Clerics and holds a Zap; the last seat holds a Zap too.
fn alchemist_table(seats: usize, clerics: usize) -> Scenario {
    let mut mine = vec![
        ready(card("Battletide Alchemist").unwrap()),
        ready(mountain()),
    ];
    mine.extend((0..clerics).map(|i| ready(cleric(&format!("Priest {i}")))));
    Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [burn("Zap", 3)])
        .battlefield(0, mine)
}

#[rstest]
fn r110_battletide_alchemist_may_prevent_as_much_as_its_clerics(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] clerics: usize,
    #[values(true, false)] prevent: bool,
    #[values(0, 1)] victim: usize,
) -> Result<(), ScriptError> {
    let setup = alchemist_table(seats, clerics).setup().unwrap();
    let mut driver = Driver::from_scenario(alchemist_table(seats, clerics))?;
    let target = driver.target_player(victim);
    let before = life(&driver, victim);
    driver.cast_at("Zap", &[target])?;
    settle(&mut driver, prevent)?;
    let prevented = if prevent { 1 + clerics } else { 0 };
    assert_eq!(
        life(&driver, victim),
        before - 3 + i32::try_from(prevented).unwrap()
    );
    replays(&driver, &setup);
    Ok(())
}

/// Seat 0 controls Magma Pummeler with `counters` +1/+1 counters and holds a burn spell of `damage` with a Mountain.
fn pummeler_table(seats: usize, counters: u32, damage: i32) -> Scenario {
    let pummeler = with_counters(
        ready(card("Magma Pummeler").unwrap()),
        CounterKind::PlusOnePlusOne,
        counters,
    );
    Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [burn("Blast", damage)])
        .battlefield(0, [pummeler, ready(mountain())])
}

#[rstest]
fn r110_magma_pummeler_turns_counters_into_damage(
    #[values(2, 3, 4)] seats: usize,
    #[values((3, 2), (2, 5), (1, 1))] case: (u32, i32),
) -> Result<(), ScriptError> {
    let (counters, damage) = case;
    let setup = pummeler_table(seats, counters, damage).setup().unwrap();
    let mut driver = Driver::from_scenario(pummeler_table(seats, counters, damage))?;
    let pummeler = driver.target_named("Magma Pummeler")?;
    let foe = driver.target_player(seats - 1);
    let before = life(&driver, seats - 1);
    driver.cast_at("Blast", &[pummeler])?;
    // The reflexive trigger goes on the stack once the spell has resolved; its target is the last seat.
    let mut aimed = false;
    for _ in 0..200 {
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        if idle && aimed {
            break;
        }
        if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
            driver.choose_targets(&[foe])?;
            aimed = true;
        } else {
            driver.answer_passively()?;
        }
    }
    let removed = counters.min(u32::try_from(damage).unwrap());
    if removed < counters {
        let id = driver.id_of("Magma Pummeler")?;
        let held = driver.state().object(id).unwrap().permanent().unwrap();
        assert_eq!(held.damage(), 0, "the damage is prevented");
        assert_eq!(
            held.counters_of(&CounterKind::PlusOnePlusOne),
            counters - removed
        );
    }
    assert_eq!(
        life(&driver, seats - 1),
        before - i32::try_from(removed).unwrap()
    );
    replays(&driver, &setup);
    Ok(())
}
