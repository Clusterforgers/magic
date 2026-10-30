//! Goal: the Auras of the owner's deck that change what a creature is play as printed at tables of two, three and four seats: Traveler's Cloak asks its controller for a land type as it enters (the land types of CR 205.3i), remembers it, draws a card and gives the creature it enchants landwalk of that type, so the defender can't block it while they control a land of the type and can when they don't; Unable to Scream makes the creature it enchants a Toy artifact creature 0/2 with no abilities that keeps its other types, and a face-down creature it enchants can't be turned face up while it is enchanted.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::{CardType, LAND_TYPES, Subtype};
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{ChoiceOption, OptionChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::keywords::keywords_of;
use mtg_engine::object::Chosen;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, island, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, lands};
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

/// Passes until the stack is empty again.
fn da_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// A table where seat 0 holds `cards`, `forests` Forests and an Island in the first main phase, and the other seats a Bear.
fn da_unable_table(seats: usize, cards: &[Arc<CardDefinition>], forests: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, cards.iter().cloned())
        .battlefield(
            0,
            lands(forests, mtg_testkit::cards::forest)
                .into_iter()
                .chain([ready(island())]),
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Bear", 2, 2))]);
    }
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| free_creature("Filler", 1, 1)));
    }
    scenario
}

/// The face-down permanent on the battlefield.
fn da_face_down(driver: &Driver) -> ObjectId {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().face_down().is_some())
        .expect("a face-down permanent")
}

/// The face-down permanents the holder of priority may turn face up now.
fn da_turnable(driver: &Driver) -> Vec<ObjectId> {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.turnable.clone(),
        _ => Vec::new(),
    }
}

/// Seat 0 casts the Aura `name` at the object `target` and lets it resolve.
fn da_enchant(driver: &mut Driver, name: &str, target: ObjectId) -> Result<(), ScriptError> {
    let target = Target::Object(driver.state().object(target).unwrap().reference());
    driver.begin_cast(name)?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    da_settle(driver)
}

#[rstest]
#[case::enchanted(true)]
#[case::not_enchanted(false)]
fn unable_to_scream_keeps_a_face_down_creature_from_being_turned_face_up(
    #[values(2, 3, 4)] seats: usize,
    #[case] enchanted: bool,
) -> Result<(), ScriptError> {
    let cards = |seats| {
        da_unable_table(
            seats,
            &[
                card("Nantuko Vigilante").unwrap(),
                card("Unable to Scream").unwrap(),
            ],
            5,
        )
    };
    let setup = cards(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(cards(seats))?;
    driver.begin_cast("Nantuko Vigilante")?;
    choose_way(&mut driver, "Cast face down")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    da_settle(&mut driver)?;
    let id = da_face_down(&driver);
    if enchanted {
        da_enchant(&mut driver, "Unable to Scream", id)?;
        assert_eq!(da_turnable(&driver), Vec::<ObjectId>::new());
        let refused = driver.answer(Answer::Priority(PriorityAction::TurnFaceUp(id)));
        assert!(refused.is_err(), "the action is not offered");
        assert!(driver.state().object(id).unwrap().face_down().is_some());
    } else {
        assert_eq!(da_turnable(&driver), [id]);
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn unable_to_scream_makes_a_face_up_creature_a_toy_with_no_abilities(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = |seats| {
        da_unable_table(seats, &[card("Unable to Scream").unwrap()], 0)
            .battlefield(0, [ready(island()), ready(card("Serra Angel").unwrap())])
    };
    let mut driver = Driver::from_scenario(table(seats))?;
    let angel = driver.id_of("Serra Angel")?;
    let base = characteristics(driver.state(), angel).unwrap();
    assert_eq!((base.power, base.toughness), (Some(4), Some(4)));
    assert_eq!(keywords_of(driver.state(), angel).len(), 2);
    da_enchant(&mut driver, "Unable to Scream", angel)?;
    let chars = characteristics(driver.state(), angel).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(0), Some(2)));
    assert!(chars.types.contains(CardType::Artifact) && chars.types.contains(CardType::Creature));
    for kind in ["Angel", "Toy"] {
        let printed = &driver.state().object(angel).unwrap().card().subtypes;
        assert!(
            chars.has_subtype(printed, &Subtype(kind.to_owned())),
            "{kind}"
        );
    }
    assert_eq!(keywords_of(driver.state(), angel).len(), 0);
    // The enchanted creature isn't face down, so nothing stops it from being what it is.
    assert!(driver.state().object(angel).unwrap().face_down().is_none());
    Ok(())
}
