//! Goal: Tamiyo Meets the Story Circle's chapter I plays at two to four seats: each creature that attacks its controller
//! gets -2/-0 until end of turn, until its controller's next turn, and an attack on another player is left alone.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::characteristics::characteristics;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

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
