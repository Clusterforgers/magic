//! Goal: "tap target creature an opponent controls. That creature doesn't untap during its controller's next untap step" (Niblis of Frost) keeps the creature tapped through the next untap step of its controller and no longer (CR 502.3), at tables of two, three and four seats with the creature on the next seat or the last one, so the untap steps of other players do not use the rule up; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::lands;
use common::pid;
use mtg_cards::modify::Duration;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 controls Niblis of Frost and a Mountain and holds Lightning Bolt; `holder` controls Grizzly Bears.
fn game(seats: usize, holder: usize) -> Scenario {
    Scenario::new(seats)
        .hand_named(0, ["Lightning Bolt"])
        .battlefield(
            0,
            lands(0, 1)
                .into_iter()
                .chain([ready(card("Niblis of Frost").unwrap())]),
        )
        .battlefield(holder, [ready(card("Grizzly Bears").unwrap())])
}

/// Whether the Bears are tapped.
fn bears_tapped(driver: &Driver) -> bool {
    let bears = driver.id_of("Grizzly Bears").unwrap();
    driver
        .state()
        .object(bears)
        .and_then(|object| object.permanent())
        .unwrap()
        .tapped()
}

/// The effects that last through a next untap step.
fn held(driver: &Driver) -> usize {
    driver
        .state()
        .effects()
        .iter()
        .filter(|effect| effect.duration == Duration::NextUntapStep)
        .count()
}

#[rstest]
fn the_creature_stays_tapped_through_its_controllers_next_untap_step(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] next_seat: bool,
) -> Result<(), ScriptError> {
    let holder = if next_seat { 1 } else { seats - 1 };
    let setup = game(seats, holder).setup().unwrap();
    let mut driver = Driver::from_scenario(game(seats, holder))?;
    driver.pass_until(Step::Main1)?;
    let player = driver.target_player(holder);
    driver.cast_at("Lightning Bolt", &[player])?;
    // The trigger of Niblis of Frost goes on the stack above the Bolt and taps the only creature an opponent controls.
    driver.idle_until(1, Step::End)?;
    assert!(bears_tapped(&driver), "the trigger tapped the Bears");
    assert_eq!(held(&driver), 1);
    let first = u32::try_from(holder).unwrap() + 1;
    for turn in 2..first {
        driver.idle_until(turn, Step::Upkeep)?;
        assert!(bears_tapped(&driver), "turn {turn} of another seat");
        assert_eq!(
            held(&driver),
            1,
            "turn {turn} of another seat keeps the rule"
        );
    }
    driver.idle_until(first, Step::Upkeep)?;
    assert_eq!(driver.state().turn().active(), pid(holder));
    assert!(
        bears_tapped(&driver),
        "the next untap step of the controller"
    );
    assert_eq!(held(&driver), 0, "that untap step used the rule up");
    let again = first + u32::try_from(seats).unwrap();
    driver.idle_until(again, Step::Upkeep)?;
    assert!(
        !bears_tapped(&driver),
        "the untap step after that untaps them"
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
