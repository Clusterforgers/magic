//! Goal: the keywords the macro table expands play whole at tables of two, three and four seats as their rules say: bushido pumps its creature by its number when it blocks or becomes blocked and not when it is unblocked, and mobilize attacks with its number of tapped Warriors that are sacrificed at the next end step.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_engine::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, keyworded};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn macro_keyword(name: &str, amount: u32) -> Keyword {
    Keyword::Macro {
        name: name.to_owned(),
        amount,
    }
}

/// Seat 0 controls these attackers, ready; every other seat has a Wall that deals no damage.
fn table(
    seats: usize,
    attackers: Vec<Arc<CardDefinition>>,
    walls: &[Arc<CardDefinition>],
) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, attackers.into_iter().map(ready).collect::<Vec<_>>());
    for seat in 1..seats {
        let own: Vec<_> = if seat == 1 {
            walls.iter().cloned().map(ready).collect()
        } else {
            vec![ready(free_creature(&format!("Wall {seat}"), 0, 4))]
        };
        scenario = scenario.battlefield(seat, own);
    }
    scenario
}

/// Seat 0 attacks seat 1 with `attacks`; the wall named `blocker` blocks `blocked` when both are given. The game goes
/// on until `until` with an empty stack.
fn fight(
    driver: &mut Driver,
    attacks: &[&str],
    block: Option<(&str, &str)>,
    until: Step,
) -> Result<(), ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    let aimed: Vec<(&str, usize)> = attacks.iter().map(|name| (*name, 1)).collect();
    driver.attack(&aimed)?;
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::DeclareBlockers(_) => {
                let chosen: Vec<(&str, &str)> = if decision.player == pid(1) {
                    block.into_iter().collect()
                } else {
                    Vec::new()
                };
                driver.block(&chosen)?;
            }
            DecisionKind::Priority(_)
                if driver.state().turn().step() == until && driver.state().stack().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the game did not reach {until:?}");
}

fn size(driver: &Driver, name: &str) -> (i32, i32) {
    let id = driver.id_of(name).unwrap();
    let chars = characteristics(driver.state(), id).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// How many permanents the seat controls.
fn permanents(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().controller() == pid(seat))
        .count()
}

#[rstest]
fn bushido_pumps_a_blocked_attacker_by_its_number(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] amount: u32,
) -> Result<(), ScriptError> {
    let samurai = keyworded("Samurai", 1, 3, &[macro_keyword("bushido", amount)]);
    let mut driver =
        Driver::from_scenario(table(seats, vec![samurai], &[free_creature("Wall", 0, 4)]))?;
    fight(
        &mut driver,
        &["Samurai"],
        Some(("Wall", "Samurai")),
        Step::EndCombat,
    )?;
    let amount = i32::try_from(amount).unwrap();
    assert_eq!(size(&driver, "Samurai"), (1 + amount, 3 + amount));
    Ok(())
}

#[rstest]
fn bushido_pumps_a_blocker_by_its_number(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] amount: u32,
) -> Result<(), ScriptError> {
    let warrior = keyworded("Warrior", 0, 4, &[macro_keyword("bushido", amount)]);
    let attacker = free_creature("Attacker", 0, 4);
    let mut driver = Driver::from_scenario(table(seats, vec![attacker], &[warrior]))?;
    fight(
        &mut driver,
        &["Attacker"],
        Some(("Warrior", "Attacker")),
        Step::EndCombat,
    )?;
    let amount = i32::try_from(amount).unwrap();
    assert_eq!(size(&driver, "Warrior"), (amount, 4 + amount));
    assert_eq!(size(&driver, "Attacker"), (0, 4));
    Ok(())
}

#[rstest]
fn bushido_does_nothing_for_an_unblocked_creature(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let samurai = keyworded("Samurai", 1, 3, &[macro_keyword("bushido", 2)]);
    let mut driver =
        Driver::from_scenario(table(seats, vec![samurai], &[free_creature("Wall", 0, 4)]))?;
    fight(&mut driver, &["Samurai"], None, Step::EndCombat)?;
    assert_eq!(size(&driver, "Samurai"), (1, 3));
    Ok(())
}

#[rstest]
fn mobilize_attacks_with_tapped_warriors_that_are_sacrificed_at_the_end_step(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] amount: u32,
) -> Result<(), ScriptError> {
    let raider = keyworded("Raider", 1, 1, &[macro_keyword("mobilize", amount)]);
    let mut driver =
        Driver::from_scenario(table(seats, vec![raider], &[free_creature("Wall", 0, 4)]))?;
    fight(&mut driver, &["Raider"], None, Step::EndCombat)?;
    let tokens = usize::try_from(amount).unwrap();
    assert_eq!(permanents(&driver, 0), 1 + tokens);
    // The Raider and every Warrior attacked the same player unblocked.
    assert_eq!(life(&driver, 1), 20 - 1 - i32::try_from(amount).unwrap());
    for seat in 2..seats {
        assert_eq!(life(&driver, seat), 20, "seat {seat}");
    }
    fight_on(&mut driver, Step::End)?;
    assert_eq!(permanents(&driver, 0), 1);
    Ok(())
}

/// Passes on until `until` with an empty stack.
fn fight_on(driver: &mut Driver, until: Step) -> Result<(), ScriptError> {
    for _ in 0..80 {
        if let DecisionKind::Priority(_) = &driver.decision()?.kind
            && driver.state().turn().step() == until
            && driver.state().stack().is_empty()
        {
            return Ok(());
        }
        if let DecisionKind::Priority(_) = &driver.decision()?.kind {
            driver.pass()?;
        } else {
            driver.answer_passively()?;
        }
    }
    panic!("the game did not reach {until:?}");
}
