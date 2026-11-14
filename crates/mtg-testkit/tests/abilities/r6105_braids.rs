//! Goal: at the beginning of its controller's end step Braids, Arisen Nightmare lets them sacrifice a permanent of the
//! five types; then each opponent in turn order may sacrifice a permanent sharing a card type with it, and each who
//! doesn't loses 2 life while Braids's controller draws a card; nothing happens without the first sacrifice, at two to
//! four seats; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// How many permanents `seat` controls.
fn r6105_permanents(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.controller() == pid(seat))
        .count()
}

#[rstest]
fn r6105_braids_punishes_each_opponent_who_doesnt(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] sacrifice: bool,
) -> Result<(), ScriptError> {
    // Seat 1 has a land and sacrifices it; seat 2 has only a creature; seat 3 has a land and keeps it.
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main2)
        .battlefield(0, [ready(card("Braids, Arisen Nightmare").unwrap()), ready(plains())])
        .library(0, vec![bear(); 5]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [if seat == 2 { ready(bear()) } else { ready(plains()) }]);
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    let life: Vec<i32> = (0..seats)
        .map(|seat| driver.state().player(pid(seat)).unwrap().life())
        .collect();
    let mut asked = Vec::new();
    for _ in 0..200 {
        if driver.state().turn().step() == Step::Cleanup || driver.state().turn().number() > 1 {
            break;
        }
        let decision = driver.decision()?.clone();
        let seat = (0..seats).find(|seat| pid(*seat) == decision.player).unwrap();
        match &decision.kind {
            DecisionKind::YesNo(options) => {
                asked.push((seat, options.only_no));
                let yes = if seat == 0 { sacrifice } else { seat == 1 };
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::ChooseCards(options) => {
                // Braids's controller sacrifices the Plains, never Braids.
                let plains = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| driver.state().object(*id).unwrap().card().name == "Plains")
                    .collect::<Vec<_>>();
                driver.answer(Answer::ChooseCards(CardsChoice(plains[..1].to_vec())))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let punished: Vec<usize> = if sacrifice { (2..seats).collect() } else { Vec::new() };
    for seat in 1..seats {
        let lost = life[seat] - driver.state().player(pid(seat)).unwrap().life();
        assert_eq!(lost, if punished.contains(&seat) { 2 } else { 0 }, "seat {seat}");
    }
    assert_eq!(r6105_permanents(&driver, 0), if sacrifice { 1 } else { 2 });
    assert_eq!(r6105_permanents(&driver, 1), usize::from(!sacrifice));
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand + punished.len());
    if sacrifice && seats > 2 {
        // Seat 2 controls no land: it can only decline.
        assert!(asked.iter().any(|(seat, only_no)| *seat == 2 && *only_no) || !asked.iter().any(|(seat, _)| *seat == 2));
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
