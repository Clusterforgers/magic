//! Goal: Power Leak's upkeep trigger plays at two to four seats: the enchanted enchantment's controller chooses how much
//! mana to pay (none up to all they can), the Aura deals them 2 damage, X of it is prevented for the X mana paid, the
//! prevention covers only that damage (no shield is left over when more was paid than prevented), and each game replays
//! to its live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, attached_to, ready};
use rstest::rstest;

/// Seat 1 controls Glorious Anthem and three Islands; seat 0 enchants the Anthem with Power Leak. Plays to seat 1's
/// first main phase, paying `paid` mana at the trigger; returns the most the number choice offered.
fn leak(seats: usize, paid: u32) -> (Driver, Option<u32>) {
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(
                1,
                [
                    ready(card("Glorious Anthem").unwrap()),
                    ready(island()),
                    ready(island()),
                    ready(island()),
                ],
            )
            .battlefield(0, [attached_to(ready(card("Power Leak").unwrap()), 1, 0)]),
        |scenario, seat| scenario.library(seat, vec![mountain(); 8]),
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let mut most = None;
    for _ in 0..200 {
        let (turn, step) = (driver.state().turn().number(), driver.state().turn().step());
        if turn == 2 && step == Step::Main1 {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseNumber(options) => {
                assert_eq!(
                    decision.player,
                    PlayerId(1),
                    "the enchantment's controller pays"
                );
                most = options.max;
                driver
                    .answer(Answer::ChooseNumber(NumberChoice::Number(paid)))
                    .unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    (driver, most)
}

#[rstest]
fn power_leak_prevents_as_much_as_was_paid(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] paid: u32,
) {
    let (driver, most) = leak(seats, paid);
    assert_eq!(most, Some(3), "up to the three Islands");
    let life = driver.state().player(PlayerId(1)).unwrap().life();
    assert_eq!(life, 20 - 2 + i32::try_from(paid.min(2)).unwrap());
    assert_eq!(driver.state().player(PlayerId(0)).unwrap().life(), 20);
    assert!(
        !driver
            .state()
            .rule_effects()
            .iter()
            .any(|effect| matches!(effect.rule, Rule::PreventDamage(_))),
        "the prevention covered only that damage"
    );
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}
