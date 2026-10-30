//! Goal: the Auras of the owner's deck that change what a creature is play as printed at tables of two, three and four seats: Traveler's Cloak asks its controller for a land type as it enters (the land types of CR 205.3i), remembers it, draws a card and gives the creature it enchants landwalk of that type, so the defender can't block it while they control a land of the type and can when they don't.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::{LAND_TYPES, Subtype};
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::Chosen;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, island, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::combat::to_blockers;
use crate::common::{name_of, names, on_battlefield, pid};
use mtg_engine::zones::Zone;

/// A table at the first main step of seat 0's second turn: seat 0 holds the card in hand with three Islands and a Bear,
/// seat 1 a Guard and the lands given, and every library is full.
fn da_table(seats: usize, hand: &str, defender_lands: &[&str]) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [card(hand).unwrap()])
        .battlefield(
            0,
            (0..3)
                .map(|_| ready(island()))
                .chain([ready(free_creature("Bear", 2, 2))]),
        )
        .battlefield(
            1,
            [ready(free_creature("Guard", 2, 2))]
                .into_iter()
                .chain(defender_lands.iter().map(|land| {
                    ready(match *land {
                        "Island" => island(),
                        "Swamp" => swamp(),
                        "Mountain" => mountain(),
                        other => panic!("no land called {other}"),
                    })
                })),
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| free_creature("Filler", 1, 1)));
    }
    scenario
}

/// Casts `name` at the Bear and passes until the choice of a land type is asked of seat 0, then answers with `land`.
fn da_cast_and_choose(driver: &mut Driver, name: &str, land: &str) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    let bear = driver.target_named("Bear")?;
    driver.choose_targets(&[bear])?;
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        if let DecisionKind::ChooseOption(options) = &decision.kind {
            assert_eq!(decision.player, pid(0));
            assert_eq!(options.text, "Choose a land type.");
            let labels: Vec<&str> = options
                .options
                .iter()
                .map(|option| match option {
                    ChoiceOption::Label(label) => label.as_str(),
                    other => panic!("a land type was expected, got {other:?}"),
                })
                .collect();
            assert_eq!(labels, LAND_TYPES);
            let at = labels.iter().position(|label| *label == land).unwrap();
            driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            break;
        }
        driver.pass()?;
    }
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The names of the creatures seat 1 may block "Bear" with after it attacks seat 1.
fn da_blockers_of_the_bear(driver: &mut Driver) -> Result<Vec<String>, ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Bear", 1)])?;
    to_blockers(driver, 1)?;
    let DecisionKind::DeclareBlockers(options) = &driver.decision()?.kind else {
        return Ok(Vec::new());
    };
    let mut found: Vec<String> = options
        .blockers
        .iter()
        .filter(|blocker| !blocker.can_block.is_empty())
        .map(|blocker| name_of(driver.state(), blocker.object))
        .collect();
    found.sort();
    Ok(found)
}

#[rstest]
#[case::the_defender_has_the_type("Swamp", &["Swamp"], &[])]
#[case::the_defender_has_another_type("Swamp", &["Island", "Mountain"], &["Guard"])]
#[case::the_defender_has_no_land("Swamp", &[], &["Guard"])]
#[case::the_type_among_other_lands("Mountain", &["Island", "Mountain"], &[])]
#[case::a_type_nobody_has("Desert", &["Swamp"], &["Guard"])]
fn traveler_s_cloak_gives_landwalk_of_the_chosen_land_type(
    #[values(2, 3, 4)] seats: usize,
    #[case] land: &str,
    #[case] defender_lands: &[&str],
    #[case] blockers: &[&str],
) -> Result<(), ScriptError> {
    let setup = da_table(seats, "Traveler's Cloak", defender_lands)
        .setup()
        .unwrap();
    let mut driver = Driver::from_scenario(da_table(seats, "Traveler's Cloak", defender_lands))?;
    da_cast_and_choose(&mut driver, "Traveler's Cloak", land)?;
    // The Aura entered, remembers the choice, and its enter trigger drew a card.
    let cloak = on_battlefield(driver.state(), "Traveler's Cloak").unwrap();
    assert_eq!(
        driver
            .state()
            .object(cloak)
            .unwrap()
            .permanent()
            .unwrap()
            .chosen()
            .cloned(),
        Some(Chosen::LandType(Subtype(land.to_owned())))
    );
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Filler"]);
    assert_eq!(da_blockers_of_the_bear(&mut driver)?, blockers);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
