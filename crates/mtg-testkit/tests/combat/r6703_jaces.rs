//! Goal: a rule that creatures can't attack the planeswalkers a selector picks (`Prohibited::AttackPlaneswalkers`,
//! CR 508.1b) takes only those planeswalkers from each creature's defenders, leaving players and other planeswalkers;
//! Jace, Multiverse Architect asks the active opponent to pay {2} at the beginning of their combat and makes it only
//! when they don't, at every table size.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A planeswalker of one subtype with no abilities.
fn walker(name: &str, subtype: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Planeswalker)
            .with_subtype(subtype)
            .with_loyalty(3),
    )
}

/// Seat 0 controls Jace, Multiverse Architect, another Jace and an Ajani; seat 1, whose turn it is, has a Bear and
/// `lands` Forests and answers Jace's question with `pays` (when it is asked). The Bear's defenders when attackers are
/// declared.
fn defenders(
    seats: usize,
    lands: usize,
    pays: bool,
) -> Result<(Driver, Vec<AttackTarget>), ScriptError> {
    let jace = card("Jace, Multiverse Architect").unwrap();
    let mut scenario = Scenario::new(seats)
        .at(2, 1, Step::Main1)
        .battlefield(
            0,
            [
                ready(jace),
                ready(walker("Other Jace", "Jace")),
                ready(walker("Some Ajani", "Ajani")),
            ],
        )
        .battlefield(
            1,
            std::iter::once(ready(free_creature("Bear", 2, 2)))
                .chain((0..lands).map(|_| ready(forest()))),
        );
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::YesNo(question) if question.text == "Pay {2}?" => {
                assert_eq!(decision.player, pid(1), "the active opponent is asked");
                driver.answer(Answer::YesNo(YesNoChoice(pays && !question.only_no)))?;
            }
            DecisionKind::DeclareAttackers(options) => {
                let bear = driver.id_of("Bear")?;
                let found = options
                    .attackers
                    .iter()
                    .find(|option| option.object == bear)
                    .map(|option| option.defenders.clone())
                    .unwrap_or_default();
                return Ok((driver, found));
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("attackers were never declared");
}

#[rstest]
fn the_bear_attacks_jaces_only_when_the_two_are_paid(
    #[values(2, 3, 4)] seats: usize,
    #[values((2, true, true), (2, false, false), (1, true, false), (0, false, false))] case: (
        usize,
        bool,
        bool,
    ),
) -> Result<(), ScriptError> {
    let (lands, pays, jaces_open) = case;
    let (driver, defenders) = defenders(seats, lands, pays)?;
    let walker = |name: &str| AttackTarget::Planeswalker(driver.id_of(name).unwrap());
    assert!(
        defenders.contains(&AttackTarget::Player(pid(0))),
        "the player stays open"
    );
    assert!(
        defenders.contains(&walker("Some Ajani")),
        "another planeswalker stays open"
    );
    for jace in ["Jace, Multiverse Architect", "Other Jace"] {
        assert_eq!(defenders.contains(&walker(jace)), jaces_open, "{jace}");
    }
    Ok(())
}

#[rstest]
fn jace_asks_nothing_on_its_controllers_turn(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let jace = card("Jace, Multiverse Architect").unwrap();
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [ready(jace), ready(free_creature("Bear", 2, 2))])
            .battlefield(1, [ready(walker("Their Jace", "Jace"))]),
    )?;
    driver.pass_until(Step::DeclareAttackers)?;
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("attackers expected");
    };
    let theirs = AttackTarget::Planeswalker(driver.id_of("Their Jace")?);
    assert!(
        options
            .attackers
            .iter()
            .any(|option| option.defenders.contains(&theirs))
    );
    Ok(())
}
