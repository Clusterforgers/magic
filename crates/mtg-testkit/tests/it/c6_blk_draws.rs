//! Goal: a replacement that doubles each draw of its controller "except the first one you draw in each of your draw steps" (Teferi's Ageless Insight, Alhammarret's Archive) leaves the first draw of each of their draw steps alone and doubles every later one (an additional draw in the draw step), while the plain doubling (Thought Reflection) doubles every draw, at tables of two to four seats, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;

use crate::common::pid;

/// Seat 0 controls `doubler` (and Howling Mine when `mine`) at the upkeep of its second turn; each library has ten cards.
fn table(seats: usize, doubler: &str, mine: bool) -> Scenario {
    let mut battlefield = vec![ready(corpus::card(doubler).unwrap())];
    if mine {
        battlefield.push(ready(corpus::card("Howling Mine").unwrap()));
    }
    let library = || (0..10).map(|i| free_creature(&format!("Pad {i}"), 1, 1));
    (0..seats).fold(
        Scenario::new(seats)
            .at(2, 0, Step::Upkeep)
            .battlefield(0, battlefield),
        |scenario, seat| scenario.library(seat, library()),
    )
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::insight_alone("Teferi's Ageless Insight", false, 1)]
#[case::insight_and_an_extra_draw("Teferi's Ageless Insight", true, 3)]
#[case::archive_alone("Alhammarret's Archive", false, 1)]
#[case::archive_and_an_extra_draw("Alhammarret's Archive", true, 3)]
#[case::every_draw_doubled("Thought Reflection", false, 2)]
#[case::every_draw_doubled_with_an_extra_draw("Thought Reflection", true, 4)]
fn the_first_draw_of_the_draw_step_is_spared_and_later_ones_are_doubled(
    #[case] doubler: &str,
    #[case] mine: bool,
    #[case] drawn: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats, doubler, mine).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, doubler, mine))?;
    driver.pass_until(Step::Main1)?;
    let state = driver.state();
    assert_eq!(state.zone(Zone::Hand(pid(0))).len(), drawn);
    assert_eq!(state.zone(Zone::Library(pid(0))).len(), 10 - drawn);
    for seat in 1..seats {
        assert_eq!(state.zone(Zone::Hand(pid(seat))).len(), 0);
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn an_opponents_draw_step_is_not_replaced(
    #[values("Teferi's Ageless Insight", "Alhammarret's Archive")] doubler: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || table(seats, doubler, true).at(2, 1, Step::Upkeep);
    let setup = build().setup().unwrap();
    let mut driver = Driver::from_scenario(build())?;
    driver.pass_until(Step::Main1)?;
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(1))).len(),
        2,
        "one and Howling Mine's"
    );
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), 0);
    replays(&driver, &setup);
    Ok(())
}
