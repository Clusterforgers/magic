//! Goal: Ulamog, the Defiler plays whole: its cast trigger makes the target opponent exile the top half of their library rounded up, it enters with +1/+1 counters equal to the greatest mana value among cards in exile, and its annihilator counts those counters as the trigger resolves (CR 702.86 with a counted amount); at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::lands;
use crate::common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::Target;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, giant, island, wastes};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick};
use rstest::rstest;

/// Seat 0 in its first main phase with Ulamog in hand and ten Wastes; the last seat has `library` cards.
fn cast_table(seats: usize, library: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, lands(10, wastes))
        .hand(0, [card("Ulamog, the Defiler").unwrap()]);
    for seat in 0..seats {
        let count = if seat == seats - 1 { library } else { 10 };
        scenario = scenario.library(seat, (0..count).map(|_| island()));
    }
    scenario
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
#[case::two_seats_odd_library(2, 9, 5)]
#[case::three_seats_even_library(3, 10, 5)]
#[case::four_seats_one_card(4, 1, 1)]
fn the_cast_trigger_exiles_half_rounded_up(
    #[case] seats: usize,
    #[case] library: usize,
    #[case] exiled: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(cast_table(seats, library))?;
    driver.begin_cast("Ulamog, the Defiler")?;
    for _ in 0..20 {
        if !driver.state().stack().is_empty() || driver.state().resolving().is_some() {
            break;
        }
        let decision = driver.decision()?.clone();
        if let mtg_engine::decision::DecisionKind::ChooseTargets(_) = decision.kind {
            driver.answer(mtg_engine::decision::Answer::ChooseTargets(
                mtg_engine::choose::TargetChoice::Targets(vec![Target::Player(pid(seats - 1))]),
            ))?;
        } else {
            driver.answer_passively()?;
        }
    }
    settle(&mut driver)?;
    let last = pid(seats - 1);
    assert_eq!(
        driver.state().zone(Zone::Library(last)).len(),
        library - exiled
    );
    // The greatest mana value in exile is an Island's 0, so Ulamog enters with no counters.
    let ulamog = driver.id_of("Ulamog, the Defiler")?;
    let counters = driver
        .state()
        .object(ulamog)
        .and_then(|o| o.permanent())
        .map_or(0, |p| p.counters_of(&CounterKind::PlusOnePlusOne));
    assert_eq!(counters, 0);
    Ok(())
}

#[rstest]
#[case::two_seats_two(2, 2)]
#[case::three_seats_three(3, 3)]
#[case::four_seats_one(4, 1)]
fn annihilator_counts_the_counters(
    #[case] seats: usize,
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let ulamog = mtg_testkit::with_counters(
        ready(card("Ulamog, the Defiler").unwrap()),
        CounterKind::PlusOnePlusOne,
        counters,
    );
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ulamog])
        .battlefield(1, [sick(bear()), sick(bear()), sick(bear()), sick(giant())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Ulamog, the Defiler", 1)])?;
    settle(&mut driver)?;
    let left = driver.state().zone(Zone::Battlefield).len() - 1;
    assert_eq!(left, 4 - usize::try_from(counters).unwrap());
    Ok(())
}
