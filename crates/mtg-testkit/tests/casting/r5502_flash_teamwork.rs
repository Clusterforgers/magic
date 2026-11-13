//! Goal: "You may cast this spell as though it had flash if it's cast using teamwork." plays at tables of two, three and
//! four seats: on another player's turn Quantum Reduction goes on to its payment only when its teamwork cost is paid
//! (and the creatures are tapped) and is rewound to its owner's hand otherwise; on its owner's main phase it is cast
//! either way, and the cast spell resolves onto its target.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{CardsChoice, CostsChoice, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

const NAME: &str = "Quantum Reduction";

/// Passes until `seat` has priority.
fn r5502_priority_to(driver: &mut Driver, seat: usize, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..seats {
        if driver.decision()?.player.index() == seat {
            return Ok(());
        }
        driver.pass()?;
    }
    assert_eq!(
        driver.decision()?.player.index(),
        seat,
        "the seat has priority"
    );
    Ok(())
}

#[rstest]
#[case::teamwork_on_another_turn(true, false, true)]
#[case::without_teamwork_on_another_turn(false, false, false)]
#[case::without_teamwork_on_its_own_turn(false, true, true)]
#[case::teamwork_on_its_own_turn(true, true, true)]
fn r5502_quantum_reduction_has_flash_only_with_teamwork(
    #[values(2, 3, 4)] seats: usize,
    #[case] teamwork: bool,
    #[case] own_turn: bool,
    #[case] cast: bool,
) -> Result<(), ScriptError> {
    let holder = seats - 1;
    let foe = (holder + 1) % seats;
    let active = if own_turn { holder } else { foe };
    let scenario = Scenario::new(seats)
        .at(1, active, Step::Main1)
        .hand(holder, [card(NAME).unwrap()])
        .battlefield(
            holder,
            [
                ready(island()),
                ready(island()),
                ready(free_creature("Mate", 2, 2)),
            ],
        )
        .battlefield(foe, [ready(free_creature("Brute", 6, 6))]);
    let mut driver = Driver::from_scenario(scenario)?;
    r5502_priority_to(&mut driver, holder, seats)?;
    driver.begin_cast(NAME)?;
    let paying = loop {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCosts(_) => {
                let chosen = if teamwork { vec![0] } else { vec![] };
                driver.answer(Answer::ChooseCosts(CostsChoice::Costs(chosen)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let state = driver.state();
                let mate: Vec<_> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| state.object(*id).unwrap().card().name == "Mate")
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(mate)))?;
            }
            DecisionKind::ChooseTargets(options) => {
                let state = driver.state();
                let brute = *options
                    .legal
                    .iter()
                    .find(|target| {
                        target.object().is_some_and(|object| {
                            state.object(object.id).unwrap().card().name == "Brute"
                        })
                    })
                    .expect("the Brute is a legal target");
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![brute])))?;
            }
            DecisionKind::PayMana(_) => break true,
            _ => break false,
        }
    };
    assert_eq!(paying, cast, "teamwork {teamwork}, own turn {own_turn}");
    if !paying {
        assert_eq!(names(driver.state(), Zone::Hand(pid(holder))), [NAME]);
        return Ok(());
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert_eq!(names(driver.state(), Zone::Stack), [NAME]);
    while !driver.state().stack().is_empty() {
        driver.answer_passively()?;
    }
    let state = driver.state();
    let brute = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().card().name == "Brute")
        .unwrap();
    let power = mtg_engine::characteristics::characteristics(state, brute)
        .unwrap()
        .power;
    assert_eq!(power, Some(1), "the Brute gets -5/-0");
    Ok(())
}
