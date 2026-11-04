//! Goal: provoke, rampage and enlist play whole at tables of two, three and four seats as their rules say: provoke untaps the creature it targets and only that one must then block the provoker, rampage grows by its number for each blocker beyond the first, and enlist taps a creature that is free to tap and adds that creature's power to the enlisting attacker.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::RefCell;

use crate::common::pid;
use crate::macro_keywords::{macro_keyword, play, size};
use mtg_cards::CardDefinition;
use mtg_engine::choose::{Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, BlockOptions, Decision, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, keyworded};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick, tapped};
use rstest::rstest;
use std::sync::Arc;

/// Seat 0 controls `own` (ready, unless `lines` says otherwise); seat 1 `foes`; every other seat a Wall.
fn scenario(
    seats: usize,
    own: Vec<mtg_engine::PresetPermanent>,
    foes: Vec<mtg_engine::PresetPermanent>,
) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, own)
        .battlefield(1, foes);
    for seat in 2..seats {
        scenario = scenario.battlefield(
            seat,
            vec![ready(free_creature(&format!("Wall {seat}"), 0, 4))],
        );
    }
    scenario
}

fn name_of(driver: &Driver, target: &Target) -> String {
    match target {
        Target::Object(object) => driver
            .state()
            .object(object.id)
            .unwrap()
            .card()
            .name
            .clone(),
        Target::Player(_) => "player".to_owned(),
    }
}

#[rstest]
fn rampage_grows_for_each_blocker_beyond_the_first(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] number: u32,
    #[values(0, 1, 2, 3)] blockers: usize,
) -> Result<(), ScriptError> {
    let rampager = keyworded("Rampager", 3, 20, &[macro_keyword("rampage", number)]);
    let walls: Vec<_> = (0..blockers)
        .map(|at| ready(free_creature(&format!("Wall {at}"), 0, 40)))
        .collect();
    let blocks: Vec<(String, &str)> = (0..blockers)
        .map(|at| (format!("Wall {at}"), "Rampager"))
        .collect();
    let mut driver = Driver::from_scenario(scenario(seats, vec![ready(rampager)], walls))?;
    let chosen: Vec<(&str, &str)> = blocks.iter().map(|(a, b)| (a.as_str(), *b)).collect();
    play(
        &mut driver,
        &["Rampager"],
        &chosen,
        &|_, _| None,
        Step::EndCombat,
    )?;
    let extra = i32::try_from(number).unwrap() * i32::try_from(blockers.saturating_sub(1)).unwrap();
    assert_eq!(size(&driver, "Rampager"), (3 + extra, 20 + extra));
    for at in 0..blockers {
        assert_eq!(
            size(&driver, &format!("Wall {at}")),
            (0, 40),
            "blockers are not pumped"
        );
    }
    Ok(())
}

/// Plays until seat 1 declares blockers; `says_yes` answers the provoke question and `pick` is the target.
fn until_blocks(
    driver: &mut Driver,
    pick: &str,
    says_yes: bool,
    offered: &RefCell<Vec<String>>,
) -> Result<BlockOptions, ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Provoker", 1)])?;
    for _ in 0..40 {
        let decision: Decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseTargets(options) => {
                let mut names: Vec<String> =
                    options.legal.iter().map(|t| name_of(driver, t)).collect();
                names.sort();
                *offered.borrow_mut() = names;
                let chosen = options
                    .legal
                    .iter()
                    .find(|t| name_of(driver, t) == pick)
                    .unwrap();
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![*chosen])))?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(says_yes)))?;
            }
            DecisionKind::DeclareBlockers(options) if decision.player == pid(1) => {
                return Ok(options.clone());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("seat 1 was never asked to block");
}

fn provoke_table(seats: usize) -> Result<Driver, ScriptError> {
    let provoker = keyworded("Provoker", 2, 9, &[macro_keyword("provoke", 0)]);
    Driver::from_scenario(scenario(
        seats,
        vec![ready(provoker)],
        vec![
            tapped(free_creature("Wall A", 0, 4)),
            ready(free_creature("Wall B", 0, 4)),
            ready(free_creature("Wall C", 0, 4)),
        ],
    ))
}

#[rstest]
fn provoke_untaps_its_target_and_only_that_creature_must_block(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = provoke_table(seats)?;
    let offered = RefCell::new(Vec::new());
    let options = until_blocks(&mut driver, "Wall A", true, &offered)?;
    assert_eq!(
        *offered.borrow(),
        ["Wall A", "Wall B", "Wall C"],
        "only creatures the defending player controls"
    );
    let a = driver.id_of("Wall A").unwrap();
    let provoker = driver.id_of("Provoker").unwrap();
    assert!(
        !driver
            .state()
            .object(a)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    assert_eq!(options.required, [a]);
    let wall = options.blockers.iter().find(|o| o.object == a).unwrap();
    assert_eq!(wall.can_block, [provoker]);
    // The others stay free to block or not.
    assert!(
        driver.block(&[("Wall B", "Provoker")]).is_err(),
        "Wall A must block too"
    );
    assert!(driver.block(&[]).is_err());
    driver.block(&[("Wall A", "Provoker")])?;
    Ok(())
}

#[rstest]
fn a_declined_provoke_changes_nothing(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = provoke_table(seats)?;
    let offered = RefCell::new(Vec::new());
    let options = until_blocks(&mut driver, "Wall A", false, &offered)?;
    let a = driver.id_of("Wall A").unwrap();
    assert!(
        driver
            .state()
            .object(a)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    assert_eq!(options.required, Vec::<mtg_core::ids::ObjectId>::new());
    driver.block(&[])?;
    Ok(())
}

#[rstest]
fn provoke_leaves_the_other_creatures_free_when_the_provoked_one_blocks_alone(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = provoke_table(seats)?;
    let offered = RefCell::new(Vec::new());
    until_blocks(&mut driver, "Wall B", true, &offered)?;
    driver.block(&[("Wall B", "Provoker"), ("Wall C", "Provoker")])?;
    Ok(())
}

fn enlist_hook<'a>(
    pick: &'a str,
    offered: &'a RefCell<Vec<String>>,
) -> impl Fn(&Driver, &Decision) -> Option<Answer> + 'a {
    move |driver, decision| match &decision.kind {
        DecisionKind::ChooseCards(options) if decision.player == pid(0) => {
            let name = |id: &mtg_core::ids::ObjectId| {
                driver.state().object(*id).unwrap().card().name.clone()
            };
            let mut names: Vec<String> = options.from.iter().map(name).collect();
            names.sort();
            *offered.borrow_mut() = names;
            let chosen = options.from.iter().find(|id| name(id) == pick)?;
            Some(Answer::ChooseCards(mtg_engine::choose::CardsChoice(vec![
                *chosen,
            ])))
        }
        DecisionKind::YesNo(_) if decision.player == pid(0) => {
            Some(Answer::YesNo(YesNoChoice(true)))
        }
        _ => None,
    }
}

fn enlist_cards() -> Vec<mtg_engine::PresetPermanent> {
    let enlister = keyworded("Enlister", 2, 9, &[macro_keyword("enlist", 0)]);
    let another: Arc<CardDefinition> =
        keyworded("Other Enlister", 4, 4, &[macro_keyword("enlist", 0)]);
    vec![
        ready(enlister),
        ready(free_creature("Helper", 3, 3)),
        ready(free_creature("Big Helper", 5, 5)),
        sick(free_creature("Fresh", 6, 6)),
        ready(another),
        tapped(free_creature("Tired", 7, 7)),
    ]
}

#[rstest]
fn enlist_taps_a_free_creature_and_adds_its_power(
    #[values(2, 3, 4)] seats: usize,
    #[values("Helper", "Big Helper")] pick: &str,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(
        seats,
        enlist_cards(),
        vec![ready(free_creature("Wall", 0, 9))],
    ))?;
    let offered = RefCell::new(Vec::new());
    let hook = enlist_hook(pick, &offered);
    play(&mut driver, &["Enlister"], &[], &hook, Step::EndCombat)?;
    assert_eq!(
        *offered.borrow(),
        ["Big Helper", "Helper"],
        "untapped, not attacking, no enlist, not sick"
    );
    let power = if pick == "Helper" { 3 } else { 5 };
    assert_eq!(size(&driver, "Enlister"), (2 + power, 9));
    let tapped_ids = |name: &str| {
        let id = driver.id_of(name).unwrap();
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    };
    assert!(tapped_ids(pick));
    let other = if pick == "Helper" {
        "Big Helper"
    } else {
        "Helper"
    };
    assert!(!tapped_ids(other), "only the chosen creature is tapped");
    Ok(())
}
