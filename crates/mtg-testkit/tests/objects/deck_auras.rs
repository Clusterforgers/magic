//! Goal: the Auras of the owner's deck that change what a creature is play as printed at tables of two, three and four seats: Traveler's Cloak asks its controller for a land type as it enters (the land types of CR 205.3i), remembers it, draws a card and gives the creature it enchants landwalk of that type, so the defender can't block it while they control a land of the type and can when they don't; Unable to Scream makes the creature it enchants a Toy artifact creature 0/2 with no abilities that keeps its other types, and a face-down creature it enchants can't be turned face up while it is enchanted. Orzhov Charm returns a creature and the Auras its controller controls attached to it, destroys a creature and costs its caster life equal to its toughness as the layers left it, or returns a cheap creature card from the caster's graveyard.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::{CardType, LAND_TYPES, Subtype};
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{ChoiceOption, ModeChoice, OptionChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::keywords::keywords_of;
use mtg_engine::object::Chosen;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::attach_cards::{hex_aura, iron_blade, mighty_aura, winged_aura};
use mtg_testkit::cards::{free_creature, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
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

/// A table where seat 0 holds Orzhov Charm with a Plains and a Swamp and controls a Bear with its own Mighty Aura and
/// Iron Blade on it, and every other seat controls a Guard with the Hex Aura of seat 0 on it and a Winged Aura on the Bear.
fn da_charm_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [card("Orzhov Charm").unwrap()])
        .battlefield(
            0,
            [
                ready(plains()),
                ready(swamp()),
                ready(free_creature("Bear", 2, 2)),
                attached_to(ready(mighty_aura().unwrap()), 0, 2),
                attached_to(ready(iron_blade().unwrap()), 0, 2),
                // The Hex Aura of seat 0 enchants the Guard, the first permanent of seat 1.
                attached_to(ready(hex_aura().unwrap()), 1, 0),
            ],
        )
        .battlefield(
            1,
            [
                ready(free_creature("Guard", 3, 5)),
                attached_to(ready(winged_aura().unwrap()), 0, 2),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| free_creature("Filler", 1, 1)));
    }
    scenario
}

/// Casts Orzhov Charm with the mode at the creature `target` and lets it resolve.
fn da_cast_charm(driver: &mut Driver, mode: usize, target: Target) -> Result<(), ScriptError> {
    driver.begin_cast("Orzhov Charm")?;
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![mode])))?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    da_settle(driver)
}

fn da_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
fn orzhov_charm_returns_the_creature_and_the_auras_its_controller_controls_attached_to_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = da_charm_table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(da_charm_table(seats))?;
    let bear = driver.target_named("Bear")?;
    da_cast_charm(&mut driver, 0, bear)?;
    // The Bear and the Aura seat 0 controls on it are in the hand of their owner.
    let mut hand = names(driver.state(), Zone::Hand(pid(0)));
    hand.sort();
    assert_eq!(hand, ["Bear", "Mighty Aura"]);
    // What is not an Aura of seat 0 on the Bear stays: the Equipment, and the Hex Aura on the Guard.
    assert!(on_battlefield(driver.state(), "Iron Blade").is_some());
    assert!(
        driver
            .state()
            .host_of(driver.id_of("Iron Blade")?)
            .is_none()
    );
    assert_eq!(
        driver.state().host_of(driver.id_of("Hex Aura")?),
        Some(driver.id_of("Guard")?)
    );
    // The Aura of another player lost its host and went to its owner's graveyard.
    assert!(on_battlefield(driver.state(), "Winged Aura").is_none());
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(1))),
        ["Winged Aura"]
    );
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Orzhov Charm"]
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
// A 3/5 Guard with the -2/-2 of a Hex Aura is a 1/3; a 2/2 Bear with the +2/+2 of a Mighty Aura and the +1/+1 of an Iron Blade is a 5/5.
#[case::an_opponents_creature_with_an_aura_that_shrinks_it("Guard", 17)]
#[case::the_casters_own_creature_grown_by_its_attachments("Bear", 15)]
fn orzhov_charm_destroys_the_creature_and_its_controller_loses_life_equal_to_its_toughness(
    #[values(2, 3, 4)] seats: usize,
    #[case] victim: &str,
    #[case] life_left: i32,
) -> Result<(), ScriptError> {
    let setup = da_charm_table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(da_charm_table(seats))?;
    let target = driver.target_named(victim)?;
    da_cast_charm(&mut driver, 1, target)?;
    // The caster loses the life, whoever controlled the creature, by its toughness as the layers left it.
    assert_eq!(da_life(&driver, 0), life_left);
    for seat in 1..seats {
        assert_eq!(da_life(&driver, seat), 20, "seat {seat}");
    }
    assert!(on_battlefield(driver.state(), victim).is_none());
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn orzhov_charm_returns_a_creature_card_of_mana_value_one_or_less_from_the_graveyard_of_its_controller(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = |seats| {
        da_charm_table(seats)
            .graveyard(
                0,
                [
                    free_creature("Free", 1, 1),
                    card("Grizzly Bears").unwrap(),
                    card("Llanowar Elves").unwrap(),
                ],
            )
            .graveyard(1, [free_creature("Their Free", 1, 1)])
    };
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.begin_cast("Orzhov Charm")?;
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![2])))?;
    // Only the creature cards of mana value 1 or less of the caster's own graveyard may be chosen.
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the target is asked");
    };
    let mut legal: Vec<String> = options
        .legal
        .iter()
        .map(|target| match target {
            Target::Object(object) => name_of(driver.state(), object.id),
            Target::Player(_) => panic!("a card was expected"),
        })
        .collect();
    legal.sort();
    assert_eq!(legal, ["Free", "Llanowar Elves"]);
    let elves = da_grave(&driver, 0, "Llanowar Elves");
    driver.choose_targets(&[elves])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    da_settle(&mut driver)?;
    assert!(on_battlefield(driver.state(), "Llanowar Elves").is_some());
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(1))),
        ["Their Free"]
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

/// The card of this name in the graveyard of `seat`, as a target.
fn da_grave(driver: &Driver, seat: usize, name: &str) -> Target {
    let state = driver.state();
    let id = state
        .zone(Zone::Graveyard(pid(seat)))
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().card().name == name)
        .unwrap();
    Target::Object(state.object(id).unwrap().reference())
}

#[rstest]
fn a_land_type_that_is_not_on_offer_is_refused_and_changes_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(da_table(seats, "Traveler's Cloak", &[]))?;
    driver.begin_cast("Traveler's Cloak")?;
    let bear = driver.target_named("Bear")?;
    driver.choose_targets(&[bear])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..20 {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
            break;
        }
        driver.pass()?;
    }
    let before = driver.state().state_hash();
    let refused = driver.answer(Answer::ChooseOption(OptionChoice(LAND_TYPES.len())));
    assert!(refused.is_err());
    assert_eq!(driver.state().state_hash(), before);
    // The question is still there, and the right answer goes through.
    assert!(matches!(
        driver.decision()?.kind,
        DecisionKind::ChooseOption(_)
    ));
    driver.answer(Answer::ChooseOption(OptionChoice(0)))?;
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
        Some(Chosen::LandType(Subtype(LAND_TYPES[0].to_owned())))
    );
    Ok(())
}
