//! Goal: token copies made with a granted keyword and a granted triggered ability (Face Yourself) copy each creature the
//! target player controls, have haste, and are sacrificed at the beginning of the end step only while their controller
//! controls no planeswalker, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_engine::choose::TargetChoice;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::keywords::keywords_of;
use mtg_engine::state::GameState;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::pid;

/// The tokens seat 0 controls.
fn r110_tokens(state: &GameState) -> Vec<mtg_core::ids::ObjectId> {
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            state.object(*id).is_some_and(|object| {
                object.kind() == mtg_engine::object::ObjectKind::Token
                    && object.controller() == pid(0)
            })
        })
        .collect()
}

#[rstest]
fn r110_each_creature_is_copied_with_haste_and_a_sacrifice(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] planeswalker: bool,
) {
    let last = seats - 1;
    let mut own: Vec<_> = (0..7).map(|_| ready(mountain())).collect();
    if planeswalker {
        own.push(ready(card("Jace Beleren").unwrap()));
    }
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, own)
        .battlefield(
            last,
            [
                ready(free_creature("Bear A", 2, 2)),
                ready(free_creature("Bear B", 3, 3)),
            ],
        )
        .hand(0, [card("Face Yourself").unwrap()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Face Yourself").unwrap();
    driver
        .answer(Answer::ChooseTargets(TargetChoice::Targets(vec![
            mtg_engine::choose::Target::Player(pid(last)),
        ])))
        .unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let tokens = r110_tokens(driver.state());
    let mut names: Vec<String> = tokens
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect();
    names.sort();
    assert_eq!(names, ["Bear A", "Bear B"]);
    for id in &tokens {
        assert!(keywords_of(driver.state(), *id).contains(&&Keyword::Haste));
    }
    driver.idle_until(3, Step::Upkeep).unwrap();
    let left = r110_tokens(driver.state()).len();
    assert_eq!(left, if planeswalker { 2 } else { 0 });
}
