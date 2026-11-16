//! Goal: an X the text defines ("{X}{X}, {T}: ... X is the number of pin counters on this artifact", Voodoo Doll, CR 107.3c) is never chosen: the activation asks no number, costs twice the pin counters, and is not offered when that much can't be paid; the Doll's end step trigger destroys it while untapped and deals its pin counters to its controller; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::lands;
use crate::common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{island, wastes};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped, with_counters};
use rstest::rstest;

/// The Doll's activated ability: its third printed ability.
const PING: usize = 2;

fn pin() -> CounterKind {
    CounterKind::Other("pin".to_owned())
}

/// Seat 0 in its first main phase with a Voodoo Doll holding `pins` pin counters (tapped when `tapped_doll`) and
/// `mana` Wastes.
fn table(seats: usize, pins: u32, mana: usize, tapped_doll: bool) -> Scenario {
    let doll = card("Voodoo Doll").unwrap();
    let doll = if tapped_doll {
        tapped(doll)
    } else {
        ready(doll)
    };
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [vec![with_counters(doll, pin(), pins)], lands(mana, wastes)].concat(),
    );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    scenario
}

#[rstest]
#[case::two_seats_two_pins(2, 2, 4, true)]
#[case::three_seats_three_pins_short(3, 3, 5, false)]
#[case::four_seats_one_pin(4, 1, 2, true)]
fn the_ping_costs_twice_the_pins_and_asks_no_x(
    #[case] seats: usize,
    #[case] pins: u32,
    #[case] mana: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, pins, mana, false))?;
    let abilities = driver.offered_abilities("Voodoo Doll")?;
    assert!(abilities.contains(&PING), "{abilities:?}");
    if !offered {
        // Twice the pins can't be paid: the announcement backs out and nothing is paid (CR 601.2h, 602.2).
        let _ = driver.begin_ability("Voodoo Doll", PING);
        assert!(driver.state().stack().is_empty());
        assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
        return Ok(());
    }
    let before = driver.state().player(pid(seats - 1)).unwrap().life();
    driver.begin_ability("Voodoo Doll", PING)?;
    for _ in 0..12 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseNumber(_) => panic!("X is not chosen"),
            DecisionKind::ChooseTargets(_) => {
                let at = Target::Player(pid(seats - 1));
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![at])))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
                break;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let untapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.card().name == "Wastes")
        .filter(|object| object.permanent().is_some_and(|p| !p.tapped()))
        .count();
    assert_eq!(
        untapped,
        mana - 2 * usize::try_from(pins).unwrap(),
        "{{X}}{{X}} paid"
    );
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    let life = driver.state().player(pid(seats - 1)).unwrap().life();
    assert_eq!(before - life, i32::try_from(pins).unwrap());
    Ok(())
}

#[rstest]
#[case::two_seats_untapped(2, false)]
#[case::three_seats_tapped(3, true)]
#[case::four_seats_untapped(4, false)]
fn the_end_step_destroys_an_untapped_doll(
    #[case] seats: usize,
    #[case] tapped_doll: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 3, 0, tapped_doll))?;
    let before = driver.state().player(pid(0)).unwrap().life();
    driver.idle_until(2, Step::Upkeep)?;
    let alive = driver.id_of("Voodoo Doll").is_ok();
    assert_eq!(alive, tapped_doll);
    let lost = before - driver.state().player(pid(0)).unwrap().life();
    assert_eq!(lost, if tapped_doll { 0 } else { 3 });
    Ok(())
}
