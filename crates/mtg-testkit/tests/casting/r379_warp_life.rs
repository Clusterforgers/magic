//! Goal: a warp cost with a life part ("Warp—{B}, Pay 2 life") is cast at two to four seats from the hand and, for
//! Timeline Culler, from the graveyard: the caster pays the mana and the life, the creature enters, and it is exiled at
//! the beginning of the next end step; without black mana it is not cast; the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::choose_way;
use crate::common::pid;

/// Seat 0 holds Timeline Culler in hand or graveyard, beside one land.
fn r379_table(seats: usize, in_graveyard: bool, black: bool) -> Scenario {
    let land = if black { swamp() } else { mountain() };
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(land)]);
    let culler = card("Timeline Culler").unwrap();
    if in_graveyard {
        scenario.graveyard(0, [culler])
    } else {
        scenario.hand(0, [culler])
    }
}

fn r379_where(driver: &Driver) -> Option<Zone> {
    let state = driver.state();
    [
        Zone::Battlefield,
        Zone::Exile,
        Zone::Hand(pid(0)),
        Zone::Graveyard(pid(0)),
    ]
    .into_iter()
    .find(|zone| {
        state
            .zone(*zone)
            .iter()
            .any(|id| state.object(*id).unwrap().card().name == "Timeline Culler")
    })
}

#[rstest]
#[case::from_hand(false)]
#[case::from_graveyard(true)]
fn r379_timeline_culler_warps_for_mana_and_life(
    #[values(2, 3, 4)] seats: usize,
    #[case] in_graveyard: bool,
) -> Result<(), ScriptError> {
    let table = || r379_table(seats, in_graveyard, true);
    let mut driver = Driver::from_scenario(table())?;
    driver.begin_cast("Timeline Culler")?;
    for _ in 0..10 {
        if !driver.state().stack().is_empty() {
            break;
        }
        match &driver.decision()?.kind {
            DecisionKind::ChooseOption(_) => choose_way(&mut driver, "Warp")?,
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                let decision = driver.decision()?.clone();
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    for _ in 0..8 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    assert_eq!(r379_where(&driver), Some(Zone::Battlefield));
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life(),
        18,
        "2 life paid"
    );
    driver.idle_until(3, Step::Upkeep)?;
    assert_eq!(
        r379_where(&driver),
        Some(Zone::Exile),
        "exiled at the end step"
    );
    let setup = table().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
#[case::near_miss_from_hand(false)]
#[case::near_miss_from_graveyard(true)]
fn r379_without_black_mana_it_is_not_cast(
    #[values(2, 3, 4)] seats: usize,
    #[case] in_graveyard: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r379_table(seats, in_graveyard, false))?;
    assert!(driver.begin_cast("Timeline Culler").is_err());
    Ok(())
}
