//! Goal: Desolation plays as printed at two to four seats: at each end step only the players who tapped a land for
//! mana this turn sacrifice a land, and only those who sacrificed a Plains take 2 damage; a player who tapped nothing
//! keeps their lands and life.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// The lands on the battlefield the seat owns.
fn r6401_lands(driver: &Driver, seat: usize) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let name = name_of(state, **id);
            (name == "Plains" || name == "Forest")
                && state.object(**id).unwrap().owner() == pid(seat)
        })
        .count()
}

/// The seat's life.
fn r6401_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
#[case::a_plains_tapped_and_sacrificed(Some(Color::White), true, 2)]
#[case::a_forest_tapped_and_sacrificed(Some(Color::Green), false, 0)]
#[case::nothing_tapped(None, false, 0)]
fn r6401_desolation_takes_a_land_from_who_tapped_one(
    #[values(2, 3, 4)] seats: usize,
    #[case] spent: Option<Color>,
    #[case] own_plains: bool,
    #[case] damage: i32,
) -> Result<(), ScriptError> {
    let land = if own_plains { plains() } else { forest() };
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Desolation").unwrap()), ready(land)]);
    if let Some(color) = spent {
        scenario = scenario.hand(0, [creature("Squire", 0, &[color], 1, 1)]);
    }
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(plains())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let life: Vec<i32> = (0..seats).map(|seat| r6401_life(&driver, seat)).collect();
    if spent.is_some() {
        driver.cast("Squire")?;
        while matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            driver.answer(Answer::PayMana(PayChoice::Auto))?;
        }
    }
    driver.idle_until(2, Step::Upkeep)?;
    let tapped = spent.is_some();
    assert_eq!(r6401_lands(&driver, 0), usize::from(!tapped));
    assert_eq!(r6401_life(&driver, 0), life[0] - damage);
    for (seat, before) in life.iter().enumerate().skip(1) {
        assert_eq!(r6401_lands(&driver, seat), 1, "seat {seat} tapped nothing");
        assert_eq!(r6401_life(&driver, seat), *before);
    }
    Ok(())
}
