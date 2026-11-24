//! Goal: Kira Nerys, Firebrand makes a creature its controller has enter while they control an attacking creature enter
//! tapped and attacking, at two to four seats: with one opponent it attacks that player unasked, with more its controller
//! chooses which opponent as it enters; outside combat it enters untapped and attacks nothing; Kira's trigger gives it
//! +1/+1; each game replays to its live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Seat 0 controls Kira, a Bear and an Island and holds Spectral Sailor ({U}, flash), at the beginning of combat or in
/// its first main phase.
fn table(seats: usize, step: Step) -> Driver {
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .at(1, 0, step)
            .battlefield(
                0,
                [
                    ready(card("Kira Nerys, Firebrand").unwrap()),
                    ready(creature("Bear", 1, &[], 2, 2)),
                    ready(island()),
                ],
            )
            .hand(0, [card("Spectral Sailor").unwrap()]),
        |scenario, seat| scenario.library(seat, vec![island(); 8]),
    );
    Driver::from_scenario(scenario).unwrap()
}

/// Answers passively until seat 0 is asked `kind`.
fn until(driver: &mut Driver, kind: &str) {
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        if decision.player == PlayerId(0) && decision.kind.name() == kind {
            return;
        }
        driver.answer(Driver::passive_answer(&decision)).unwrap();
    }
    panic!("never asked {kind}");
}

/// Resolves the stack, answering a question of what the Sailor attacks with `defender`.
fn resolve(driver: &mut Driver, defender: usize) -> bool {
    let mut asked = false;
    for _ in 0..50 {
        if driver.state().stack().is_empty() {
            break;
        }
        if let DecisionKind::ChooseOption(options) = &driver.decision().unwrap().kind {
            let at = options
                .options
                .iter()
                .position(|o| {
                    *o == ChoiceOption::Player(PlayerId(u64::try_from(defender).unwrap()))
                })
                .unwrap();
            asked = true;
            driver
                .answer(Answer::ChooseOption(OptionChoice(at)))
                .unwrap();
        } else {
            driver.pass().unwrap();
        }
    }
    asked
}

fn sailor_attacks(driver: &Driver) -> Option<PlayerId> {
    let sailor = driver.id_of("Spectral Sailor").unwrap();
    driver
        .state()
        .combat()
        .attackers()
        .iter()
        .find(|a| a.object() == sailor)
        .and_then(|a| a.defender(driver.state()))
}

fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
fn kira_has_a_creature_enter_tapped_and_attacking(#[values(2, 3, 4)] seats: usize) {
    let mut driver = table(seats, Step::BeginCombat);
    until(&mut driver, "declare-attackers");
    driver.attack(&[("Bear", 1)]).unwrap();
    until(&mut driver, "priority");
    driver.cast("Spectral Sailor").unwrap();
    let defender = seats - 1;
    let asked = resolve(&mut driver, defender);
    assert_eq!(asked, seats > 2, "asked only between two or more opponents");
    let sailor = driver.id_of("Spectral Sailor").unwrap();
    let object = driver.state().object(sailor).unwrap();
    assert!(object.permanent().unwrap().tapped(), "enters tapped");
    assert_eq!(
        sailor_attacks(&driver),
        Some(PlayerId(u64::try_from(defender).unwrap()))
    );
    let power = characteristics(driver.state(), sailor).unwrap().power;
    assert_eq!(power, Some(2), "Kira's trigger gives it +1/+1");
    replays(&driver);
}

#[rstest]
fn kira_does_nothing_without_an_attacking_creature(#[values(2, 4)] seats: usize) {
    let mut driver = table(seats, Step::Main1);
    until(&mut driver, "priority");
    driver.cast("Spectral Sailor").unwrap();
    let asked = resolve(&mut driver, 1);
    assert!(!asked);
    let sailor = driver.id_of("Spectral Sailor").unwrap();
    let object = driver.state().object(sailor).unwrap();
    assert!(!object.permanent().unwrap().tapped(), "enters untapped");
    assert_eq!(sailor_attacks(&driver), None);
    replays(&driver);
}
