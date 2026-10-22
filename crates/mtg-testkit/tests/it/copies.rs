//! Goal: a permanent that enters as a copy has the copiable values of the object it copies from the moment it enters, at tables of two, three and four seats: Clone asks its controller which creature on the battlefield to copy (none is a choice, and with no creature nobody is asked), becomes that creature's name, types, power, toughness and abilities, takes none of its counters, damage or attachments, can be a copy of a copy, keeps the exceptions of its copy effect (Phantasmal Image is an Illusion that is sacrificed when it becomes a target), applies the enters replacements of the copied card, and triggers as the copied permanent; a token that is a copy has the same values; Cackling Counterpart and populate create such tokens; and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{names, pid};
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{characteristics, replay};
use mtg_testkit::cards::island;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers whatever is asked passively until the stack is empty and nothing resolves.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn lands(count: usize) -> Vec<mtg_engine::PresetPermanent> {
    (0..count).map(|_| ready(island())).collect()
}

fn permanents(seat_names: &[&str]) -> Vec<mtg_engine::PresetPermanent> {
    seat_names
        .iter()
        .map(|name| ready(card(name).unwrap()))
        .collect()
}

/// The permanents of the battlefield named `name`.
fn on_battlefield(driver: &Driver, name: &str) -> Vec<ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().card().name == name)
        .collect()
}

fn answer_cards(driver: &mut Driver, picks: &[ObjectId]) -> Result<(), ScriptError> {
    driver.answer(Answer::ChooseCards(CardsChoice(picks.to_vec())))?;
    Ok(())
}

#[rstest]
fn clone_enters_as_a_copy_of_the_creature_it_is_asked_to_copy(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut scenario = Scenario::new(seats).hand(0, [card("Clone").unwrap()]);
        scenario = scenario.battlefield(0, lands(4));
        scenario.battlefield(1, permanents(&["Hero of Bladehold"]))
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast_tapping("Clone", &["Island", "Island", "Island", "Island"])?;
    driver.pass()?;
    for seat in 1..seats {
        let _ = seat;
    }
    // The spell resolves and its controller is asked what to copy.
    while !matches!(
        driver.engine().decision().map(|d| &d.kind),
        Some(DecisionKind::ChooseCards(_))
    ) {
        driver.answer_passively()?;
    }
    let decision = driver.decision()?.clone();
    assert_eq!(decision.player, pid(0));
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        unreachable!()
    };
    let hero = on_battlefield(&driver, "Hero of Bladehold")[0];
    assert_eq!(options.from, [hero]);
    assert_eq!((options.min, options.max), (0, 1));
    assert!(driver.engine().replacing());
    answer_cards(&mut driver, &[hero])?;
    eprintln!("{:?}", driver.engine().decision());
    eprintln!("{:?}", &driver.engine().events()[driver.engine().events().len().saturating_sub(8)..]);
    settle(&mut driver)?;
    // Both are Heroes; the Clone is the one that came from the hand.
    let heroes = on_battlefield(&driver, "Hero of Bladehold");
    assert_eq!(heroes.len(), 2);
    let copy = *heroes.iter().find(|id| **id != hero).unwrap();
    let chars = characteristics(driver.state(), copy).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(3), Some(4)));
    assert!(chars.is_creature());
    let object = driver.state().object(copy).unwrap();
    assert_eq!(object.permanent().unwrap().copy().unwrap().of().id, hero);
    assert_eq!(object.whole_card().name, "Hero of Bladehold");
    assert!(names(driver.state(), Zone::Hand(pid(0))).is_empty());
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
