//! Goal: a leaves-the-battlefield trigger of a permanent that a spell exiled and put back face down as a new object (Become
//! Anonymous cloaks the target) still reads the abilities it had on the battlefield, not those of the face-down permanent
//! that took its place (CR 400.7, 603.10a), at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn a_cloaked_creature_still_draws_for_its_leaves_trigger(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            (0..4)
                .map(|_| ready(island()))
                .chain([with_counters(
                    ready(card("Toothy, Imaginary Friend").unwrap()),
                    CounterKind::PlusOnePlusOne,
                    2,
                )])
                .collect::<Vec<_>>(),
        )
        .hand(0, [card("Become Anonymous").unwrap()])
        .library(0, (0..8).map(|_| forest()).collect::<Vec<_>>());
    let mut driver = Driver::from_scenario(scenario.at(1, 0, Step::Main1))?;
    let toothy = driver.target_named("Toothy, Imaginary Friend")?;
    driver.cast_at("Become Anonymous", &[toothy])?;
    for _ in 0..60 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    let state = driver.state();
    let seat0 = mtg_core::ids::PlayerId::from_index(0);
    assert!(
        state
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| state.object(*id))
            .any(|o| o.face_down().is_some() && o.controller() == seat0),
        "the creature came back face down"
    );
    assert_eq!(
        state.zone(Zone::Hand(seat0)).len(),
        2,
        "its trigger drew a card for each of the two counters it had"
    );
    Ok(())
}
