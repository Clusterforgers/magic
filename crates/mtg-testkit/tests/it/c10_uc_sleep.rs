//! Goal: "Tap all creatures target player controls. Those creatures don't untap during that player's next untap step." (Sleep) taps the creatures of the target player and holds them through that player's next untap step and no longer (CR 502.3, 611.2c), at tables of two, three and four seats with the target on the next seat or the last one; the log replays to the live state.
use crate::common;

use common::copying::lands;
use common::pid;
use mtg_engine::turn::Step;
use mtg_engine::{PresetPermanent, replay};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 has four Islands and holds Sleep; `holder` controls Grizzly Bears.
fn c10_uc_game(seats: usize, holder: usize, bears: PresetPermanent) -> Scenario {
    Scenario::new(seats)
        .hand_named(0, ["Sleep"])
        .battlefield(0, lands(4, 0))
        .battlefield(holder, [bears])
}

/// Whether the Bears are tapped.
fn c10_uc_tapped(driver: &Driver) -> Option<bool> {
    let bears = driver.id_of("Grizzly Bears").ok()?;
    driver
        .state()
        .object(bears)
        .and_then(|object| object.permanent())
        .map(mtg_engine::object::PermanentState::tapped)
}

#[rstest]
fn c10_uc_the_players_creatures_stay_tapped_through_that_players_next_untap_step(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] next_seat: bool,
) -> Result<(), ScriptError> {
    let holder = if next_seat { 1 } else { seats - 1 };
    let setup = c10_uc_game(seats, holder, ready(card("Grizzly Bears").unwrap()))
        .setup()
        .unwrap();
    let mut driver = Driver::from_scenario(c10_uc_game(
        seats,
        holder,
        ready(card("Grizzly Bears").unwrap()),
    ))?;
    driver.pass_until(Step::Main1)?;
    let player = driver.target_player(holder);
    driver.cast_at("Sleep", &[player])?;
    driver.idle_until(1, Step::End)?;
    assert_eq!(c10_uc_tapped(&driver), Some(true), "Sleep tapped the Bears");
    let first = u32::try_from(holder).unwrap() + 1;
    for turn in 2..first {
        driver.idle_until(turn, Step::Upkeep)?;
        assert_eq!(
            c10_uc_tapped(&driver),
            Some(true),
            "turn {turn} of another seat"
        );
    }
    driver.idle_until(first, Step::Upkeep)?;
    assert_eq!(driver.state().turn().active(), pid(holder));
    assert_eq!(
        c10_uc_tapped(&driver),
        Some(true),
        "the next untap step of that player"
    );
    let again = first + u32::try_from(seats).unwrap();
    driver.idle_until(again, Step::Upkeep)?;
    assert!(
        c10_uc_tapped(&driver) == Some(false),
        "the untap step after that untaps them"
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
