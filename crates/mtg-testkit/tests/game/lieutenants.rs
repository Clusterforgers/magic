//! Goal: the Lieutenants that give a bonus and an ability in quotes (Demon of Wailing Agonies, Tyrant's Familiar, Stormsurge Kraken) or a bonus and a keyword for a group (Angelic Field Marshal, Thunderfoot Baloth) get them only while their controller controls their commander, at tables of two, three and four seats: sizes, the keywords of the group, and the granted triggers that sacrifice a creature and deal damage to a creature of the defending player.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{name_of, names};
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::color::Color;
use mtg_engine::characteristics;
use mtg_engine::choose::{CardsChoice, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::format::Commander;
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

fn card(name: &str) -> Result<Arc<CardDefinition>, ScriptError> {
    mtg_testkit::corpus::card(name).map_err(|error| ScriptError::Stuck(error.to_string()))
}

/// Seat 0 on turn in its first main phase with the lieutenant and a Cub, two Mountains and a commander in the command
/// zone; every other seat has two 2/2 Bystanders. With `commander` the commander is cast and resolved.
fn table(seats: usize, lieutenant: &str, commander: bool) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [legend("Cmd", 1, &[Color::Red], 2, 2)])
        .battlefield(
            0,
            [
                ready(card(lieutenant)?),
                ready(free_creature("Cub", 1, 1)),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .at(1, 0, Step::Main1);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(free_creature(&format!("Bystander {seat}a"), 2, 2)),
                ready(free_creature(&format!("Bystander {seat}b"), 2, 2)),
            ],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    if commander {
        driver.cast("Cmd")?;
        for _ in 0..seats {
            driver.pass()?;
        }
    }
    Ok(driver)
}

fn size(driver: &Driver, name: &str) -> (i32, i32) {
    let chars = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

fn has(driver: &Driver, name: &str, keyword: &Keyword) -> bool {
    keywords_of(driver.state(), driver.id_of(name).unwrap()).contains(&keyword)
}

#[rstest]
#[case::demon("Demon of Wailing Agonies", 4)]
#[case::familiar("Tyrant's Familiar", 5)]
#[case::kraken("Stormsurge Kraken", 5)]
#[case::marshal("Angelic Field Marshal", 3)]
#[case::baloth("Thunderfoot Baloth", 5)]
fn a_lieutenant_gets_two_more_with_its_commander_alone(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] commander: bool,
    #[case] name: &str,
    #[case] base: i32,
) -> Result<(), ScriptError> {
    let driver = table(seats, name, commander)?;
    let grown = base + if commander { 2 } else { 0 };
    assert_eq!(size(&driver, name), (grown, grown), "{name}");
    Ok(())
}

#[rstest]
fn the_marshal_gives_vigilance_to_the_creatures_of_its_controller_alone(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] commander: bool,
) -> Result<(), ScriptError> {
    let driver = table(seats, "Angelic Field Marshal", commander)?;
    for name in ["Cub", "Angelic Field Marshal"] {
        assert_eq!(has(&driver, name, &Keyword::Vigilance), commander, "{name}");
    }
    assert!(!has(&driver, "Bystander 1a", &Keyword::Vigilance));
    Ok(())
}

#[rstest]
fn the_baloth_makes_the_other_creatures_of_its_controller_bigger_and_trampling(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] commander: bool,
) -> Result<(), ScriptError> {
    let driver = table(seats, "Thunderfoot Baloth", commander)?;
    let more = if commander { 2 } else { 0 };
    assert_eq!(size(&driver, "Cub"), (1 + more, 1 + more));
    assert_eq!(has(&driver, "Cub", &Keyword::Trample), commander);
    assert_eq!(size(&driver, "Bystander 1a"), (2, 2));
    assert!(!has(&driver, "Bystander 1a", &Keyword::Trample));
    assert!(has(&driver, "Thunderfoot Baloth", &Keyword::Trample));
    Ok(())
}

/// Passes through the combat of seat 0 until its end step begins, choosing the first card or target offered.
fn through_combat(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let state = driver.state();
        if state.turn().step() == Step::EndCombat {
            return Ok(());
        }
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseCards(options) => {
                driver.answer(Answer::ChooseCards(CardsChoice(vec![options.from[0]])))?;
            }
            DecisionKind::ChooseTargets(options) => {
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![
                    options.legal[0],
                ])))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("combat did not end".to_owned()))
}

#[rstest]
fn the_demon_makes_the_player_it_hurts_sacrifice_a_creature_with_its_commander_alone(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] commander: bool,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, "Demon of Wailing Agonies", commander)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Demon of Wailing Agonies", 1)])?;
    through_combat(&mut driver)?;
    let left = names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| name.starts_with("Bystander 1"))
        .count();
    assert_eq!(left, if commander { 1 } else { 2 });
    Ok(())
}

#[rstest]
fn the_familiar_hits_a_creature_of_the_defending_player_alone(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, "Tyrant's Familiar", true)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Tyrant's Familiar", 1)])?;
    while !matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        driver.answer_passively()?;
    }
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        unreachable!();
    };
    let named: Vec<String> = options
        .legal
        .iter()
        .filter_map(|target| target.object())
        .map(|object| name_of(driver.state(), object.id))
        .collect();
    assert_eq!(named, ["Bystander 1a", "Bystander 1b"]);
    Ok(())
}
