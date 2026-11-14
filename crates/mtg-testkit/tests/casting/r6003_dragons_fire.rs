//! Goal: Dragon's Fire deals 3 damage unless its caster beheld a Dragon as an additional cost, in which case it deals
//! damage equal to that Dragon's power, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_engine::choose::{CardsChoice, CostsChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::name_of;

/// A 5/5 Dragon creature.
fn r6003_dragon() -> std::sync::Arc<mtg_cards::CardDefinition> {
    std::sync::Arc::new(
        (*creature("Dragon", 0, &[Color::Red], 5, 5))
            .clone()
            .with_subtype("Dragon"),
    )
}

/// Seat 0 casts Dragon's Fire at seat 1's 4/4, beholding its Dragon or not; whether the 4/4 died.
fn r6003_fire(seats: usize, behold: bool) -> Result<bool, ScriptError> {
    let mut permanents = lands(2, mountain);
    permanents.push(ready(r6003_dragon()));
    let mut scenario = Scenario::new(seats)
        .battlefield(0, permanents)
        .hand(0, vec![card("Dragon's Fire").unwrap()]);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            vec![ready(creature("Ogre", 0, &[Color::Green], 4, 4))],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Dragon's Fire")?;
    for _ in 0..64 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().zone(Zone::Stack).is_empty() => break,
            DecisionKind::ChooseCosts(_) => Answer::ChooseCosts(CostsChoice::Costs(if behold {
                vec![0]
            } else {
                Vec::new()
            })),
            DecisionKind::ChooseCards(options) => {
                let dragon = options
                    .from
                    .iter()
                    .copied()
                    .find(|id| name_of(driver.state(), *id) == "Dragon")
                    .unwrap();
                Answer::ChooseCards(CardsChoice(vec![dragon]))
            }
            DecisionKind::ChooseTargets(_) => {
                let target = driver.target_named("Ogre")?;
                driver.choose_targets(&[target])?;
                continue;
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    let state = driver.state();
    Ok(state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(state, **id) == "Ogre")
        .count()
        < seats - 1)
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r6003_a_beheld_dragon_sets_the_damage(#[case] seats: usize) -> Result<(), ScriptError> {
    assert!(r6003_fire(seats, true)?, "5 damage kills the 4/4");
    assert!(!r6003_fire(seats, false)?, "3 damage leaves the 4/4");
    Ok(())
}
