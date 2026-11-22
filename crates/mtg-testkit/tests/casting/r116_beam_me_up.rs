//! Goal: beam me up casts a card from its owner's graveyard for its beam me up cost and the return of a creature its caster controls to its owner's hand, exiles the spell as it resolves, is not offered without a creature to return or from a hand, and the log replays to the same state at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CastWay;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent, replay};
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, lands, pay};
use crate::common::{name_of, names, pid};

fn card(name: &str) -> Arc<CardDefinition> {
    corpus::card(name).unwrap()
}

/// Seat 0 holds `hand`, has `graveyard` in its graveyard and controls `creatures` and `count` Islands; seat 1 controls
/// a Serra Angel.
fn game(
    seats: usize,
    hand: &[&str],
    graveyard: &[&str],
    creatures: &[&str],
    count: usize,
) -> Result<Driver, ScriptError> {
    let battlefield: Vec<PresetPermanent> = creatures
        .iter()
        .map(|name| ready(card(name)))
        .chain(lands(count, island))
        .collect();
    let scenario = (0..seats)
        .fold(Scenario::new(seats), |scenario, seat| {
            scenario.library(seat, (0..8).map(|_| forest()))
        })
        .hand_named(0, hand.to_vec())
        .graveyard_named(0, graveyard.to_vec())
        .battlefield(0, battlefield)
        .battlefield(1, vec![ready(card("Serra Angel"))]);
    Driver::from_scenario(scenario)
}

/// Whether seat 0 may cast the card called `name` now.
fn castable(driver: &Driver, name: &str) -> bool {
    match &driver.decision().map(|decision| decision.kind.clone()) {
        Ok(DecisionKind::Priority(options)) => options
            .castable
            .iter()
            .any(|id| name_of(driver.state(), *id) == name),
        _ => false,
    }
}

/// Casts Energize the beam me up way, returns `returned` for the cost, aims at the Serra Angel, pays and resolves.
fn beam(driver: &mut Driver, returned: &str) -> Result<(), ScriptError> {
    let picked = driver.id_of(returned).unwrap();
    driver.begin_cast("Energize")?;
    for _ in 0..12 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseOption(_) => choose_way(driver, "Beam me up")?,
            DecisionKind::ChooseCards(_) => {
                driver.answer(Answer::ChooseCards(CardsChoice(vec![picked])))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let giant = driver.target_named("Serra Angel")?;
                driver.choose_targets(&[giant])?;
            }
            DecisionKind::PayMana(_) => {
                pay(driver)?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Ok(())
}

#[rstest]
fn r116_beam_me_up_casts_from_the_graveyard_returning_a_creature(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, &[], &["Energize"], &["Grizzly Bears"], 4)?;
    assert!(castable(&driver, "Energize"));
    beam(&mut driver, "Grizzly Bears")?;
    let state = driver.state();
    assert_eq!(names(state, Zone::Hand(pid(0))), ["Grizzly Bears"]);
    assert_eq!(names(state, Zone::Hand(pid(1))), ["Serra Angel"]);
    assert_eq!(names(state, Zone::Exile), ["Energize"]);
    let ways: Vec<Option<CastWay>> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::SpellCast { choices, .. } => Some(choices.way),
            _ => None,
        })
        .collect();
    assert_eq!(ways, [Some(CastWay::R116BeamMeUp)]);
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
#[case::no_creature_to_return(&[], &["Energize"], &[], 4)]
#[case::not_enough_mana(&[], &["Energize"], &["Grizzly Bears"], 3)]
fn r116_beam_me_up_is_not_offered_without_its_cost(
    #[case] hand: &[&str],
    #[case] graveyard: &[&str],
    #[case] creatures: &[&str],
    #[case] count: usize,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = game(seats, hand, graveyard, creatures, count)?;
    assert!(!castable(&driver, "Energize"));
    Ok(())
}

#[rstest]
fn r116_from_a_hand_it_is_cast_for_its_mana_cost_and_goes_to_the_graveyard(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, &["Energize"], &[], &["Grizzly Bears"], 4)?;
    driver.begin_cast("Energize")?;
    for _ in 0..12 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseOption(_) => panic!("only the mana cost is open from a hand"),
            DecisionKind::ChooseTargets(_) => {
                let giant = driver.target_named("Serra Angel")?;
                driver.choose_targets(&[giant])?;
            }
            DecisionKind::PayMana(_) => {
                pay(&mut driver)?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), ["Energize"]);
    assert_eq!(
        names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|n| *n == "Grizzly Bears")
            .count(),
        1
    );
    Ok(())
}
