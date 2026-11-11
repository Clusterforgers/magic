//! Goal: a rule an effect makes about its target holds for that target only: Vexing Shusher's "target spell can't be countered" keeps Memory Lapse from countering that spell and no other, and Turf Wound's "target player can't play lands this turn" stops that player and no other, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::removal::spell_named;
use common::{pid, spell};
use mtg_core::card::CardType;
use mtg_engine::Answer;
use mtg_engine::decision::PayChoice;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn pass_until_priority(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    for _ in 0..16 {
        if driver.decision()?.player == pid(seat) {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck(format!("priority for seat {seat}")))
}

#[rstest]
#[case::shushed_it_resolves(true)]
#[case::unshushed_it_is_countered(false)]
fn a_shushed_spell_cant_be_countered(
    #[case] shush: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let victim = spell("Victim", CardType::Instant, "[]", "[]");
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [victim])
            .battlefield(
                0,
                [ready(card("Vexing Shusher").unwrap()), ready(mountain())],
            )
            .hand_named(1, ["Memory Lapse"])
            .battlefield(1, [ready(island()), ready(island())]),
    )?;
    driver.cast("Victim")?;
    let target = spell_named(&driver, "Victim");
    if shush {
        driver.begin_activation("Vexing Shusher")?;
        driver.choose_targets(&[target])?;
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        // The ability resolves first; Victim stays on the stack.
        while driver.state().stack().len() > 1 {
            driver.pass()?;
        }
    }
    pass_until_priority(&mut driver, 1)?;
    driver.cast_at("Memory Lapse", &[target])?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let graveyard = if shush { vec!["Victim"] } else { Vec::new() };
    driver.expect_zone(Zone::Graveyard(pid(0)), &graveyard)?;
    driver.expect_zone(Zone::Graveyard(pid(1)), &["Memory Lapse"])
}

#[rstest]
#[case::the_target_cant_play_lands(0, false)]
#[case::another_player_is_named(1, true)]
fn turf_wound_stops_its_target_from_playing_lands(
    #[case] named: usize,
    #[case] can_play: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [card("Turf Wound").unwrap(), forest()])
            .battlefield(0, [ready(mountain()), ready(mountain()), ready(mountain())]),
    )?;
    let target = driver.target_player(named);
    driver.cast_at("Turf Wound", &[target])?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(driver.play_land("Forest").is_ok(), can_play);
    Ok(())
}
