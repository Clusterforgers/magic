//! Goal: the cards a player surveilled this turn (`Pred::R117SurveilledBy`, CR 701.25) are counted as they land in the graveyard: Eye of Duskmantle lets its controller play a land and cast a spell from among them for life equal to the mana value (CR 118.9), while a card that was already in the graveyard or that another player surveilled stays out of reach, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat `caster` holds Consider and an Island; seat 0 controls Eye of Duskmantle; seat `caster`'s library holds
/// `top` then a Forest; seat 0's graveyard holds an Old Bear.
fn table(seats: usize, caster: usize, top: &str) -> Scenario {
    let top = if top == "Forest" {
        forest()
    } else {
        creature(top, 1, &[Color::Green], 2, 2)
    };
    let mut eye = vec![ready(card("Eye of Duskmantle").unwrap())];
    if caster == 0 {
        eye.push(ready(island()));
    }
    let mut scenario = Scenario::new(seats)
        .at(1, caster, Step::Main1)
        .battlefield(0, eye);
    if caster != 0 {
        scenario = scenario.battlefield(caster, [ready(island())]);
    }
    scenario
        .hand(caster, [card("Consider").unwrap()])
        .library(caster, [top, forest()])
        .graveyard(0, [creature("Old Bear", 1, &[Color::Green], 2, 2)])
}

fn named(driver: &Driver, name: &str) -> ObjectId {
    driver
        .state()
        .objects()
        .find(|found| found.card().name == name)
        .map(mtg_engine::object::GameObject::id)
        .unwrap()
}

/// Casts Consider and puts the surveilled card into the graveyard.
fn consider(driver: &mut Driver, top: &str) -> Result<(), ScriptError> {
    driver.cast("Consider")?;
    for _ in 0..20 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::ChooseCards(options) => {
                let id = options
                    .from
                    .iter()
                    .copied()
                    .find(|id| driver.state().object(*id).unwrap().card().name == top)
                    .unwrap();
                driver.answer(Answer::ChooseCards(CardsChoice(vec![id])))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("Consider did not resolve");
}

fn castable(driver: &Driver, name: &str) -> bool {
    let id = named(driver, name);
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => {
            options.castable.contains(&id) || options.playable_lands.contains(&id)
        }
        _ => false,
    }
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(seat))
        .unwrap()
        .life()
}

#[rstest]
fn a_surveilled_spell_is_cast_for_life(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 0, "Young Bear"))?;
    assert!(!castable(&driver, "Old Bear"), "a card not surveilled");
    consider(&mut driver, "Young Bear")?;
    assert!(castable(&driver, "Young Bear"));
    assert!(!castable(&driver, "Old Bear"));
    driver.begin_cast("Young Bear")?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    assert_eq!(life(&driver, 0), 18, "life equal to its mana value");
    while !driver.state().stack().is_empty() {
        driver.answer_passively()?;
    }
    assert!(driver.id_of("Young Bear").is_ok(), "the Bear resolved");
    Ok(())
}

#[rstest]
fn a_surveilled_land_is_played(#[values(2, 3)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 0, "Forest"))?;
    consider(&mut driver, "Forest")?;
    let id = driver
        .state()
        .zone(Zone::Graveyard(PlayerId::from_index(0)))
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().card().name == "Forest")
        .unwrap();
    match &driver.decision()?.kind {
        DecisionKind::Priority(options) => assert!(options.playable_lands.contains(&id)),
        other => panic!("{other:?}"),
    }
    Ok(())
}

#[rstest]
fn another_players_surveil_opens_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let caster = seats - 1;
    let mut driver = Driver::from_scenario(table(seats, caster, "Young Bear"))?;
    consider(&mut driver, "Young Bear")?;
    while driver.decision()?.player != PlayerId::from_index(0) {
        driver.pass()?;
    }
    assert!(
        !castable(&driver, "Young Bear"),
        "seat {caster} surveilled it"
    );
    Ok(())
}
