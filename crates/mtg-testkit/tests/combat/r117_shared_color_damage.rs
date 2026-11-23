//! Goal: damage a creature would deal to another creature that shares a color with it is prevented (`ReplaceWhen::R117CreaturesSharingColor`, Well-Laid Plans, CR 615) at tables of two to four seats, whoever controls the enchantment: a blocker and an attacker of one color deal each other nothing, of different colors or colorless they fight as usual, and damage to the defending player is never prevented.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_core::color::Color;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::creature;
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;

/// Seat 0 controls an Attacker of `mine`, seat 1 a Blocker of `theirs` (colorless when empty), both 3/3; seat
/// `owner` controls Well-Laid Plans.
fn table(
    seats: usize,
    owner: usize,
    mine: &[Color],
    theirs: &[Color],
) -> Result<Driver, ScriptError> {
    let plans = corpus::card("Well-Laid Plans").unwrap();
    let mut sides = [
        vec![ready(creature("Attacker", 1, mine, 3, 3))],
        vec![ready(creature("Blocker", 1, theirs, 3, 3))],
    ];
    sides[owner].push(ready(plans));
    let [attacking, defending] = sides;
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, attacking)
        .battlefield(1, defending);
    Driver::from_scenario(scenario)
}

/// The Attacker attacks seat 1, which blocks with the Blocker when `blocked`; the game goes on to the end of combat.
fn fight(driver: &mut Driver, blocked: bool) -> Result<(), ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Attacker", 1)])?;
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::DeclareBlockers(_) => {
                let chosen: Vec<(&str, &str)> = if blocked && decision.player == pid(1) {
                    vec![("Blocker", "Attacker")]
                } else {
                    Vec::new()
                };
                driver.block(&chosen)?;
            }
            DecisionKind::Priority(_)
                if driver.state().turn().step() == Step::EndCombat
                    && driver.state().stack().is_empty() =>
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
    panic!("combat did not end");
}

fn alive(driver: &Driver, name: &str) -> bool {
    driver.id_of(name).is_ok()
}

#[rstest]
#[case::one_color(&[Color::Red], &[Color::Red], true)]
#[case::one_of_two_colors(&[Color::Red, Color::Green], &[Color::Green, Color::White], true)]
#[case::different_colors(&[Color::Red], &[Color::Blue], false)]
#[case::colorless_shares_nothing(&[], &[], false)]
fn creatures_sharing_a_color_deal_each_other_nothing(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] owner: usize,
    #[case] mine: &[Color],
    #[case] theirs: &[Color],
    #[case] prevented: bool,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, owner, mine, theirs)?;
    fight(&mut driver, true)?;
    assert_eq!(alive(&driver, "Attacker"), prevented);
    assert_eq!(alive(&driver, "Blocker"), prevented);
    Ok(())
}

#[rstest]
fn damage_to_a_player_is_dealt(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = table(seats, 0, &[Color::Red], &[Color::Red])?;
    fight(&mut driver, false)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 17);
    Ok(())
}
