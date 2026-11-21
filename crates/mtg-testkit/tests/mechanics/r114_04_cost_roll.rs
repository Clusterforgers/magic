//! Goal: a die rolled as a cost (`CostItem::R114RollDie`, CR 706, 602.2h) plays at tables of two to four seats: Clay Golem's activation rolls one d8 as its costs are paid, logged before the ability is put on the stack, and the result is the X of Monstrosity X, so the Golem gets that many +1/+1 counters, becomes monstrous and its Berserk trigger destroys the target; the seed sets the result and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{bear, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn table(seats: usize, seed: u64) -> Scenario {
    let mut mine = vec![ready(card("Clay Golem").unwrap())];
    mine.extend((0..6).map(|_| ready(plains())));
    let mut scenario = Scenario::new(seats)
        .seed(seed)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(bear())]);
    }
    scenario
}

/// Answers until the stack is empty: an opponent's Bear as the target, mana paid automatically.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::ChooseTargets(_) => {
                let bear = driver.target_named("Grizzly Bears")?;
                driver.choose_targets(&[bear])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the stack did not empty");
}

#[rstest]
fn the_roll_of_the_cost_is_the_monstrosity(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3, 5, 8, 13)] seed: u64,
) -> Result<(), ScriptError> {
    let scenario = table(seats, seed);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let golem = driver.id_of("Clay Golem")?;
    let bears = driver.state().zone(Zone::Battlefield).len() - 7;
    driver.begin_activation("Clay Golem")?;
    settle(&mut driver)?;
    let events = driver.engine().events();
    let rolled: Vec<(u32, u32)> = events
        .iter()
        .filter_map(|event| match event {
            Event::DieRolled {
                player,
                sides,
                result,
                ..
            } if *player == pid(0) => Some((*sides, *result)),
            _ => None,
        })
        .collect();
    let [(8, result)] = rolled.as_slice() else {
        panic!("one d8: {rolled:?}");
    };
    assert!((1..=8).contains(result));
    let roll_at = events
        .iter()
        .position(|event| matches!(event, Event::DieRolled { .. }))
        .unwrap();
    let put_at = events
        .iter()
        .position(|event| matches!(event, Event::AbilityPutOnStack(_)))
        .unwrap();
    assert!(roll_at < put_at, "the die is rolled as the cost is paid");
    let state = driver.state();
    let permanent = state.object(golem).unwrap().permanent().unwrap();
    assert_eq!(
        permanent.counters_of(&CounterKind::PlusOnePlusOne),
        *result,
        "X is the result"
    );
    assert!(permanent.monstrous());
    // Berserk destroyed the first target offered: one permanent fewer.
    assert_eq!(state.zone(Zone::Battlefield).len(), bears + 6);
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[test]
fn the_seeds_roll_every_face() {
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..64 {
        let mut driver = Driver::from_scenario(table(2, seed)).unwrap();
        driver.begin_activation("Clay Golem").unwrap();
        settle(&mut driver).unwrap();
        seen.extend(
            driver
                .engine()
                .events()
                .iter()
                .filter_map(|event| match event {
                    Event::DieRolled { result, .. } => Some(*result),
                    _ => None,
                }),
        );
    }
    assert_eq!(seen, (1..=8).collect());
}
