//! Goal: a prevention shield divided among targets (Remedy, "prevent the next 5 damage that would be dealt this turn to any number of targets, divided as you choose") prevents each target's own share and no more, a player it did not target is dealt all the damage, and a target given the whole amount keeps it whole, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::life;
use common::spell;
use mtg_core::card::CardType;
use mtg_engine::Answer;
use mtg_engine::choose::DivideChoice;
use mtg_engine::decision::PayChoice;
use mtg_testkit::cards::plains;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 casts Remedy on the players of `shares` with those shares, then seat 1 casts a spell that deals 4 damage to
/// each player.
fn remedy_then_blast(seats: usize, shares: &[(usize, u32)]) -> Result<Driver, ScriptError> {
    let blast = spell(
        "Blast",
        CardType::Instant,
        "[]",
        r#"[{"DealDamage":{"to":"EachPlayer","amount":4}}]"#,
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Remedy"])
            .hand(1, [blast])
            .battlefield(0, [ready(plains()), ready(plains()), ready(plains())]),
    )?;
    driver.begin_cast("Remedy")?;
    let targets: Vec<_> = shares
        .iter()
        .map(|(seat, _)| driver.target_player(*seat))
        .collect();
    driver.choose_targets(&targets)?;
    if shares.len() > 1 {
        driver.answer(Answer::DivideAmount(DivideChoice::Division(
            targets
                .iter()
                .zip(shares)
                .map(|(target, (_, share))| (*target, *share))
                .collect(),
        )))?;
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    for _ in 0..16 {
        if driver.decision()?.player == common::pid(1) {
            break;
        }
        driver.pass()?;
    }
    driver.cast("Blast")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(driver)
}

#[rstest]
#[case::split_three_and_two(&[(1, 3), (0, 2)], &[(0, 18), (1, 19)])]
#[case::split_four_and_one(&[(1, 4), (0, 1)], &[(0, 17), (1, 20)])]
#[case::one_target_keeps_all_five(&[(1, 5)], &[(0, 16), (1, 20)])]
fn each_target_is_spared_its_own_share(
    #[case] shares: &[(usize, u32)],
    #[case] lives: &[(usize, i32)],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = remedy_then_blast(seats, shares)?;
    for (seat, expected) in lives {
        assert_eq!(life(&driver, *seat), *expected, "seat {seat}");
    }
    for seat in 2..seats {
        assert_eq!(life(&driver, seat), 16, "seat {seat} was not a target");
    }
    Ok(())
}
