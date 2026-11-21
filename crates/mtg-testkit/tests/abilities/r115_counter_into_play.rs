//! Goal: a countering by a spell its controller controls is replaced (CR 701.6, 614.1) and the cast in its place goes
//! on as part of the counterspell's resolution (CR 608.2g) at 2 to 4 seats: under Guile, a creature spell Counterspell
//! would counter is exiled and its card may be cast free by Guile's controller (cast: it resolves under them; declined:
//! it stays exiled); each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::{names, pid};

/// Seat 1, active, holds Grizzly Bears with two Forests; seat 0 controls Guile and two Islands and holds Counterspell.
fn table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .at(2, 1, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Guile").unwrap()),
                ready(island()),
                ready(island()),
            ],
        )
        .hand(0, [card("Counterspell").unwrap()])
        .battlefield(1, [ready(forest()), ready(forest())])
        .hand(1, [card("Grizzly Bears").unwrap()])
}

/// The Grizzly Bears in `zone`.
fn named(driver: &Driver, zone: Zone) -> mtg_core::ids::ObjectId {
    *driver
        .state()
        .zone(zone)
        .iter()
        .find(|id| driver.state().object(**id).unwrap().card().name == "Grizzly Bears")
        .unwrap()
}

#[rstest]
#[case::cast(true)]
#[case::declined(false)]
fn a_countered_spell_is_exiled_and_may_be_played_free(
    #[values(2, 3, 4)] seats: usize,
    #[case] cast: bool,
) -> Result<(), ScriptError> {
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.cast("Grizzly Bears")?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    let spell = named(&driver, Zone::Stack);
    let bears =
        mtg_engine::choose::Target::Object(driver.state().object(spell).unwrap().reference());
    driver.cast_at("Counterspell", &[bears])?;
    let mut asked = 0;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::YesNo(_) => {
                assert_eq!(decision.player, pid(0));
                asked += 1;
                driver.answer(Answer::YesNo(YesNoChoice(cast)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(asked, 1);
    let bears = [Zone::Battlefield, Zone::Exile]
        .into_iter()
        .find(|zone| names(driver.state(), *zone).contains(&"Grizzly Bears".to_owned()))
        .map(|zone| named(&driver, zone))
        .unwrap();
    let object = driver.state().object(bears).unwrap();
    if cast {
        assert_eq!(object.zone(), Zone::Battlefield);
        assert_eq!(object.controller(), pid(0));
    } else {
        assert_eq!(object.zone(), Zone::Exile);
    }
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(1))).len(), 0);
    replays(&driver, &setup);
    Ok(())
}
