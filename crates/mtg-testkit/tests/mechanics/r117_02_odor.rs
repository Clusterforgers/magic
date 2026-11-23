//! Goal: Olinda the Oblivious plays at three and four seats: one target creature of each opponent gets an odor counter and has "At the beginning of your upkeep, you lose 2 life" for as long as it has one (`Duration::WhileHasCounter` for each creature apart), so each opponent loses 2 life in their own upkeep and the controller never; each game replays to its live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, swamp};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn on_battlefield(driver: &Driver, name: &str) -> Vec<Target> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name == name)
        .map(|object| Target::Object(object.reference()))
        .collect()
}

#[rstest]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn each_smelly_creature_costs_its_controller_two_life_each_upkeep(#[case] seats: usize) {
    let olinda = corpus::card("Olinda the Oblivious").unwrap();
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, vec![olinda])
        .battlefield(0, (0..4).map(|_| ready(swamp())).collect::<Vec<_>>());
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| forest()));
        if seat > 0 {
            scenario = scenario.battlefield(seat, vec![ready(free_creature("Foe", 2, 2))]);
        }
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Olinda the Oblivious").unwrap();
    for _ in 0..80 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(_) => {
                Answer::ChooseTargets(TargetChoice::Targets(on_battlefield(&driver, "Foe")))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::Priority(_) => {
                if driver.state().stack().is_empty()
                    && !on_battlefield(&driver, "Olinda the Oblivious").is_empty()
                {
                    break;
                }
                driver.pass().unwrap();
                continue;
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    driver
        .idle_until(2 + u32::try_from(seats).unwrap(), Step::Main1)
        .unwrap();
    for seat in 0..seats {
        let life = driver.state().player(PlayerId(seat as u64)).unwrap().life();
        assert_eq!(life, if seat == 0 { 20 } else { 18 }, "seat {seat}");
    }
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}
