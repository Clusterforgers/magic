//! Goal: Mirror Match plays as printed at two to four seats: cast in the declare blockers step, it makes one token copy of
//! each creature attacking its caster, each blocking the creature it copies (CR 509.1h), none for a creature attacking
//! another player, and exiles those tokens at end of combat.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::DecisionKind;
use mtg_engine::object::ObjectKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// The tokens on the battlefield the seat controls, by name.
fn r6501_tokens(driver: &Driver, seat: usize) -> Vec<String> {
    let state = driver.state();
    let mut names: Vec<String> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let object = state.object(**id).unwrap();
            object.kind() == ObjectKind::Token && object.controller() == pid(seat)
        })
        .map(|id| name_of(state, *id))
        .collect();
    names.sort();
    names
}

/// The names of the creatures blocking the attacker named `name`.
fn r6501_blockers_of(driver: &Driver, name: &str) -> Vec<String> {
    let state = driver.state();
    state
        .combat()
        .attackers()
        .iter()
        .filter(|attack| name_of(state, attack.object()) == name)
        .flat_map(|attack| attack.blockers().iter().map(|id| name_of(state, *id)))
        .collect()
}

#[rstest]
fn r6501_mirror_match_blocks_each_attacker_of_its_caster_with_its_copy(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let defender = seats - 1;
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(free_creature("Raider", 2, 4)),
                ready(free_creature("Pouncer", 1, 3)),
                ready(free_creature("Roamer", 2, 3)),
            ],
        )
        .battlefield(defender, (0..6).map(|_| ready(island())))
        .hand(defender, [card("Mirror Match").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let life: Vec<i32> = (0..seats)
        .map(|seat| driver.state().player(pid(seat)).unwrap().life())
        .collect();
    driver.pass_until(Step::DeclareAttackers)?;
    // With a third seat, the Roamer attacks seat 1 and gets no copy.
    let other = if seats > 2 { 1 } else { defender };
    driver.attack(&[
        ("Raider", defender),
        ("Pouncer", defender),
        ("Roamer", other),
    ])?;
    let mut cast = false;
    let mut seen_blocks = false;
    for _ in 0..120 {
        let decision = driver.decision()?.clone();
        let step = driver.state().turn().step();
        match &decision.kind {
            DecisionKind::DeclareBlockers(_) => {
                driver.block(&[])?;
            }
            DecisionKind::Priority(_)
                if !cast && step == Step::DeclareBlockers && decision.player == pid(defender) =>
            {
                driver.cast_tapping("Mirror Match", &["Island"; 6])?;
                cast = true;
            }
            DecisionKind::Priority(_) if step == Step::CombatDamage && !seen_blocks => {
                seen_blocks = true;
                let copied = if seats > 2 {
                    vec!["Pouncer", "Raider"]
                } else {
                    vec!["Pouncer", "Raider", "Roamer"]
                };
                assert_eq!(r6501_tokens(&driver, defender), copied);
                assert_eq!(r6501_blockers_of(&driver, "Raider"), ["Raider"]);
                assert_eq!(r6501_blockers_of(&driver, "Pouncer"), ["Pouncer"]);
                if seats > 2 {
                    assert_eq!(r6501_blockers_of(&driver, "Roamer").len(), 0);
                }
                driver.pass()?;
            }
            DecisionKind::Priority(_)
                if step == Step::Main2 && driver.state().stack().is_empty() =>
            {
                break;
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert!(seen_blocks, "the game did not reach combat damage");
    assert!(
        r6501_tokens(&driver, defender).is_empty(),
        "the tokens are exiled at end of combat"
    );
    let hit = if seats > 2 { 2 } else { 0 };
    assert_eq!(
        driver.state().player(pid(defender)).unwrap().life(),
        life[defender]
    );
    if seats > 2 {
        assert_eq!(driver.state().player(pid(1)).unwrap().life(), life[1] - hit);
    }
    Ok(())
}
