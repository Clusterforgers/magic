//! Goal: two combat triggers play at two to four seats: Tamiyo Meets the Story Circle's chapter I makes each creature
//! that attacks its controller get -2/-0 until its controller's next turn and leaves attacks on other players alone;
//! Raphael, Tag Team Tough untaps the attackers and adds one more combat phase the first time it deals combat damage to
//! a player in a turn, and not again in that added combat.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::common::pid;

fn r409_power(driver: &Driver, name: &str) -> i32 {
    let id = driver.id_of(name).unwrap();
    characteristics(driver.state(), id).unwrap().power.unwrap()
}

/// Seat 0 controls Tamiyo Meets the Story Circle with no lore counter yet; seat 1 a 4/4 Brute. Chapter I triggers in
/// seat 0's first main phase; in seat 1's turn the Brute attacks `defender`.
#[rstest]
#[case::attacks_you(2, 0, 2)]
#[case::attacks_you_three_seats(3, 0, 2)]
#[case::attacks_another_player(3, 2, 4)]
#[case::attacks_you_four_seats(4, 0, 2)]
fn r409_tamiyo_shrinks_creatures_that_attack_you(
    #[case] seats: usize,
    #[case] defender: usize,
    #[case] power: i32,
) -> Result<(), ScriptError> {
    let saga = with_counters(
        ready(card("Tamiyo Meets the Story Circle").unwrap()),
        CounterKind::lore(),
        0,
    );
    let scenario = Scenario::new(seats)
        .battlefield(0, [saga])
        .battlefield(1, [ready(free_creature("Brute", 4, 4))])
        .at(1, 0, Step::Upkeep);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::DeclareAttackers)?;
    assert_eq!(r409_power(&driver, "Brute"), 4);
    driver.attack(&[("Brute", defender)])?;
    for _ in 0..10 {
        if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(r409_power(&driver, "Brute"), power);
    Ok(())
}

/// Seat 0 attacks seat 1 with Raphael and a 2/2 in every combat of turn 1; the number of combats and the damage dealt.
#[rstest]
fn r409_raphael_adds_one_combat(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(card("Raphael, Tag Team Tough").unwrap()),
                ready(free_creature("Pal", 2, 2)),
            ],
        )
        .at(1, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let raphael = r409_power(&driver, "Raphael, Tag Team Tough");
    let life = driver.state().player(pid(1)).unwrap().life();
    let mut combats = 0;
    for _ in 0..200 {
        if driver.state().turn().number() > 1 {
            break;
        }
        if matches!(driver.decision()?.kind, DecisionKind::DeclareAttackers(_)) {
            combats += 1;
            driver.attack(&[("Raphael, Tag Team Tough", 1), ("Pal", 1)])?;
        } else {
            driver.answer_passively()?;
        }
    }
    assert_eq!(
        combats, 2,
        "one added combat, and none after the second hit"
    );
    let lost = life - driver.state().player(pid(1)).unwrap().life();
    assert_eq!(lost, 2 * (raphael + 2));
    Ok(())
}
