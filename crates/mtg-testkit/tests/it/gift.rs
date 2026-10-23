//! Goal: gift plays whole at tables of two, three and four seats: the promise is offered once for each opponent and only then, the chosen opponent alone draws the card before the other effects and the spell then needs and uses its second target, a spell cast without the promise gives nothing and asks for one target, a promise to an opponent who has left gives nothing, and every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::lands;
use crate::common::{name_of, pid};
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CostsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{PresetPermanent, replay};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The log replays to the live state.
fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Seat 0 holds `hand` and controls `permanents`; every other seat controls a Bonesplitter and has a library.
fn table(seats: usize, hand: &[&str], permanents: Vec<PresetPermanent>) -> Scenario {
    let scenario = (1..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand_named(0, hand.iter().copied())
            .battlefield(0, permanents),
        |scenario, seat| {
            scenario.battlefield(
                seat,
                [
                    ready(mtg_testkit::corpus::card("Bonesplitter").unwrap()),
                    ready(free_creature("Foe", 2, 2)),
                ],
            )
        },
    );
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..8).map(|_| forest()))
    })
}

fn named(driver: &Driver, zone: Zone, name: &str) -> Vec<ObjectId> {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .copied()
        .filter(|id| name_of(state, *id) == name)
        .collect()
}

fn hand_size(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

/// Answers the promise of Wear Down: to the opponent at `seat`, or to none.
fn promise(driver: &mut Driver, seats: usize, to: Option<usize>) -> Result<(), ScriptError> {
    let DecisionKind::ChooseCosts(options) = &driver.decision()?.kind else {
        panic!("the gift is asked: {}", driver.decision()?.kind.name());
    };
    assert_eq!(
        options.optional.len(),
        seats - 1,
        "one promise for each opponent"
    );
    let costs = match to {
        None => vec![],
        Some(seat) => {
            let label = format!("to player {}", seat + 1);
            let offered = options
                .optional
                .iter()
                .find(|cost| cost.label.ends_with(&label))
                .unwrap_or_else(|| panic!("a promise {label}: {:?}", options.optional));
            assert!(offered.label.starts_with("Gift a card"));
            vec![offered.index]
        }
    };
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(costs)))?;
    Ok(())
}

/// Casts Wear Down against the artifacts, with the promise `to`, and lets it resolve.
fn cast_wear_down(
    seats: usize,
    to: Option<usize>,
    leaves: Option<usize>,
) -> Result<Driver, ScriptError> {
    let mut permanents = lands(2, forest);
    permanents.push(ready(mtg_testkit::corpus::card("Bonesplitter").unwrap()));
    let mut driver = Driver::from_scenario(table(seats, &["Wear Down"], permanents))?;
    let before: Vec<usize> = (0..seats).map(|seat| hand_size(&driver, seat)).collect();
    driver.begin_cast("Wear Down")?;
    promise(&mut driver, seats, to)?;
    let artifacts = named(&driver, Zone::Battlefield, "Bonesplitter");
    let target = |id: ObjectId| {
        mtg_engine::choose::Target::Object(driver.state().object(id).unwrap().reference())
    };
    let wanted = if to.is_some() { 2 } else { 1 };
    let picks: Vec<_> = artifacts
        .iter()
        .take(wanted)
        .map(|id| target(*id))
        .collect();
    for pick in &picks {
        assert!(matches!(
            driver.decision()?.kind,
            DecisionKind::ChooseTargets(_)
        ));
        driver.choose_targets(&[*pick])?;
    }
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    if let Some(seat) = leaves {
        driver.concede(pid(seat))?;
    }
    for _ in 0..2 * seats {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    assert!(driver.state().stack().is_empty(), "Wear Down resolved");
    let after: Vec<usize> = (0..seats).map(|seat| hand_size(&driver, seat)).collect();
    let drew: Vec<usize> = (0..seats)
        .map(|seat| after[seat].saturating_sub(before[seat] - usize::from(seat == 0)))
        .collect();
    // Seat 0 spent Wear Down from the hand; the others only gain what the gift gives.
    assert_eq!(drew[0], 0, "the caster draws nothing");
    for (seat, drew) in drew.iter().enumerate().skip(1) {
        let gifted = to == Some(seat) && leaves != Some(seat);
        assert_eq!(*drew, usize::from(gifted), "seat {seat}");
    }
    // A player who left takes their permanents with them.
    if leaves.is_none() {
        assert_eq!(
            named(&driver, Zone::Battlefield, "Bonesplitter").len(),
            artifacts.len() - wanted,
            "one artifact for each target"
        );
    }
    Ok(driver)
}

#[rstest]
#[case::not_promised(None)]
#[case::promised_to_the_first_opponent(Some(1))]
#[case::promised_to_the_last_opponent(Some(usize::MAX))]
fn the_gift_goes_to_the_chosen_opponent_alone_and_adds_the_second_target(
    #[case] to: Option<usize>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let to = to.map(|seat| seat.min(seats - 1));
    let driver = cast_wear_down(seats, to, None)?;
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::the_chosen_opponent_left(2)]
#[case::the_last_opponent_left(3)]
fn a_promise_to_an_opponent_who_left_gives_nothing(
    #[case] seat: usize,
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let seat = seat.min(seats - 1);
    let driver = cast_wear_down(seats, Some(seat), Some(seat))?;
    assert_replays(&driver);
    Ok(())
}
