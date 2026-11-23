//! Goal: a target chosen at random among the cards of a graveyard (`Targetable::AtRandom`, CR 115.7 rulings) feeds a delayed cast at the next upkeep: Power Pack's combat damage exiles one instant or sorcery card of its controller's graveyard, never another card, and at its controller's next upkeep that card may be cast free and is exiled again instead of going to the graveyard, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

/// Seat 0 controls Power Pack with `opts` Opts and a Grizzly Bears in its graveyard; every library holds Islands.
fn table(seats: usize, opts: usize) -> Scenario {
    let mut graveyard: Vec<_> = (0..opts).map(|_| card("Opt").unwrap()).collect();
    graveyard.push(bear());
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Power Pack").unwrap())])
        .graveyard(0, graveyard);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()).collect::<Vec<_>>());
    }
    scenario
}

fn named_in(driver: &Driver, zone: Zone, name: &str) -> usize {
    driver
        .state()
        .zone(zone)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == name)
        .count()
}

/// Plays on, answering `cast` to the offer of a free cast, until `turn` reaches its draw step.
fn play_until(driver: &mut Driver, turn: u32, cast: bool) -> Result<u32, ScriptError> {
    let mut offers = 0;
    for _ in 0..400 {
        if driver.state().turn().number() == turn && driver.state().turn().step() == Step::Draw {
            return Ok(offers);
        }
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::DeclareAttackers(_) if driver.state().turn().number() == 1 => {
                driver.attack(&[("Power Pack", 1)])?;
            }
            DecisionKind::YesNo(_) if decision.player == pid(0) => {
                offers += 1;
                driver.answer(Answer::YesNo(YesNoChoice(cast)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("turn {turn} was not reached");
}

#[rstest]
fn the_exiled_card_is_cast_at_the_next_upkeep(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] opts: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, opts))?;
    let next = u32::try_from(seats).unwrap() + 1;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    play_until(&mut driver, 2, true)?;
    assert_eq!(named_in(&driver, Zone::Exile, "Opt"), 1, "one Opt exiled");
    assert_eq!(named_in(&driver, Zone::Graveyard(pid(0)), "Opt"), opts - 1);
    assert_eq!(
        named_in(&driver, Zone::Graveyard(pid(0)), "Grizzly Bears"),
        1
    );
    let offers = play_until(&mut driver, next, true)?;
    assert_eq!(offers, 1);
    assert_eq!(
        named_in(&driver, Zone::Exile, "Opt"),
        1,
        "exiled instead of the graveyard"
    );
    assert_eq!(named_in(&driver, Zone::Graveyard(pid(0)), "Opt"), opts - 1);
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand + 2,
        "Opt drew a card, then the draw step did"
    );
    Ok(())
}

#[rstest]
fn a_declined_cast_leaves_the_card_in_exile(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 1))?;
    let next = u32::try_from(seats).unwrap() + 1;
    play_until(&mut driver, 2, false)?;
    play_until(&mut driver, next, false)?;
    assert_eq!(named_in(&driver, Zone::Exile, "Opt"), 1);
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 16);
    Ok(())
}
