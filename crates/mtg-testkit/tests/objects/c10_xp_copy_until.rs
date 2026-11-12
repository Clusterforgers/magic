//! Goal: "Target Shapeshifter becomes a copy of target creature until your next turn" (Shapesharer) keeps the copy through the turns of the other seats and ends it as its controller's next turn begins, when the permanent is again what it was (CR 611.2b, 707.2), at tables of two, three and four seats with the creature copied on the next seat or the last one; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::lands;
use common::pid;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::events::Event;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 controls Shapesharer and three Islands; `holder` controls Grizzly Bears.
fn game(seats: usize, holder: usize) -> Scenario {
    Scenario::new(seats)
        .battlefield(
            0,
            lands(3, 0)
                .into_iter()
                .chain([ready(card("Shapesharer").unwrap())]),
        )
        .battlefield(holder, [ready(card("Grizzly Bears").unwrap())])
}

/// The name Shapesharer shows now.
fn shown(driver: &Driver, id: mtg_core::ids::ObjectId) -> String {
    driver.state().object(id).unwrap().card().name.clone()
}

#[rstest]
fn the_copy_lasts_until_its_controllers_next_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] next_seat: bool,
) -> Result<(), ScriptError> {
    let holder = if next_seat { 1 } else { seats - 1 };
    let setup = game(seats, holder).setup().unwrap();
    let mut driver = Driver::from_scenario(game(seats, holder))?;
    driver.pass_until(Step::Main1)?;
    let sharer = driver.id_of("Shapesharer")?;
    let targets = [
        driver.target_named("Shapesharer")?,
        driver.target_named("Grizzly Bears")?,
    ];
    driver.begin_activation("Shapesharer")?;
    driver.choose_targets(&targets[..1])?;
    driver.choose_targets(&targets[1..])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    driver.idle_until(1, Step::End)?;
    assert_eq!(shown(&driver, sharer), "Grizzly Bears", "the copy began");
    let seats_u32 = u32::try_from(seats).unwrap();
    for turn in 2..=seats_u32 {
        driver.idle_until(turn, Step::Upkeep)?;
        assert_eq!(
            shown(&driver, sharer),
            "Grizzly Bears",
            "turn {turn} of another seat and its cleanup keep the copy"
        );
    }
    driver.idle_until(seats_u32 + 1, Step::Upkeep)?;
    assert_eq!(driver.state().turn().active(), pid(0));
    assert_eq!(
        shown(&driver, sharer),
        "Shapesharer",
        "the next turn ended it"
    );
    let ended = driver
        .engine()
        .events()
        .iter()
        .filter(
            |event| matches!(event, Event::CopyEnded { next_turn_of: Some(p), .. } if *p == pid(0)),
        )
        .count();
    assert_eq!(ended, 1, "one end, as the turn began");
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
