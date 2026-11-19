//! Goal: what a cast with emerge leaves for the abilities that read it (CR 702.119): Adipose Offspring cast with emerge makes as many Aliens as the sacrificed creature's toughness and one when cast for its mana cost, Foul Emissary makes its Eldrazi Horror only when its controller sacrifices it while casting a spell with emerge (not when it is sacrificed another way), at tables of two to four seats, and each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{PresetPermanent, replay};
use mtg_testkit::cards::plains;
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, lands, pay};
use crate::common::{names, spell};

fn card(name: &str) -> Arc<CardDefinition> {
    corpus::card(name).unwrap()
}

/// A free sorcery: sacrifice a creature.
fn r109_offering() -> Arc<CardDefinition> {
    spell(
        "Offering",
        CardType::Sorcery,
        "[]",
        r#"[{"Sacrifice":{"who":"You","filter":[{"Type":"Creature"}],"count":1}}]"#,
    )
}

/// Seat 0 holds `hand` and controls `creatures` and `count` Plains.
fn r109_game(
    seats: usize,
    hand: Vec<Arc<CardDefinition>>,
    creatures: &[&str],
    count: usize,
) -> Result<Driver, ScriptError> {
    let battlefield: Vec<PresetPermanent> = creatures
        .iter()
        .map(|name| ready(card(name)))
        .chain(lands(count, plains))
        .collect();
    let scenario = (0..seats)
        .fold(Scenario::new(seats), |scenario, seat| {
            scenario.library(seat, (0..8).map(|_| plains()))
        })
        .hand(0, hand)
        .battlefield(0, battlefield);
    Driver::from_scenario(scenario)
}

/// Casts `name` the way whose label starts with `way`, picks `creature` for a cost or a sacrifice when asked, pays and
/// lets the stack and the triggers resolve.
fn r109_cast(
    driver: &mut Driver,
    name: &str,
    way: &str,
    creature: Option<&str>,
) -> Result<(), ScriptError> {
    let picked = creature.map(|creature| driver.id_of(creature).unwrap());
    driver.begin_cast(name)?;
    for _ in 0..40 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseOption(_) => choose_way(driver, way)?,
            DecisionKind::ChooseCards(_) if picked.is_some() => {
                driver.answer(Answer::ChooseCards(CardsChoice(
                    picked.into_iter().collect(),
                )))?;
            }
            DecisionKind::PayMana(_) => {
                pay(driver)?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
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

fn r109_count(driver: &Driver, name: &str) -> usize {
    names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|named| *named == name)
        .count()
}

fn r109_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
// Serra Angel is a 4/4: four Aliens.
#[case::an_angel_makes_four("Serra Angel", 4)]
// Grizzly Bears is a 2/2: two Aliens.
#[case::bears_make_two("Grizzly Bears", 2)]
fn adipose_offspring_cast_with_emerge_counts_the_sacrificed_toughness(
    #[values(2, 3, 4)] seats: usize,
    #[case] sacrificed: &str,
    #[case] aliens: usize,
) -> Result<(), ScriptError> {
    let mut driver = r109_game(seats, vec![card("Adipose Offspring")], &[sacrificed], 6)?;
    r109_cast(&mut driver, "Adipose Offspring", "Emerge", Some(sacrificed))?;
    assert_eq!(r109_count(&driver, "Adipose Offspring"), 1);
    assert_eq!(r109_count(&driver, sacrificed), 0);
    assert_eq!(r109_count(&driver, "Alien"), aliens);
    r109_replays(&driver);
    Ok(())
}

#[rstest]
fn adipose_offspring_cast_for_its_mana_cost_makes_one(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r109_game(seats, vec![card("Adipose Offspring")], &["Serra Angel"], 4)?;
    r109_cast(&mut driver, "Adipose Offspring", "Cast", None)?;
    assert_eq!(r109_count(&driver, "Serra Angel"), 1);
    assert_eq!(r109_count(&driver, "Alien"), 1);
    r109_replays(&driver);
    Ok(())
}

#[rstest]
fn foul_emissary_sacrificed_for_emerge_makes_a_horror(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r109_game(
        seats,
        vec![card("Adipose Offspring")],
        &["Foul Emissary"],
        6,
    )?;
    r109_cast(
        &mut driver,
        "Adipose Offspring",
        "Emerge",
        Some("Foul Emissary"),
    )?;
    assert_eq!(r109_count(&driver, "Foul Emissary"), 0);
    assert_eq!(r109_count(&driver, "Eldrazi Horror"), 1);
    // Foul Emissary is a 1/1: one Alien.
    assert_eq!(r109_count(&driver, "Alien"), 1);
    r109_replays(&driver);
    Ok(())
}

#[rstest]
fn foul_emissary_sacrificed_otherwise_makes_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r109_game(seats, vec![r109_offering()], &["Foul Emissary"], 0)?;
    r109_cast(&mut driver, "Offering", "Cast", Some("Foul Emissary"))?;
    assert_eq!(r109_count(&driver, "Foul Emissary"), 0);
    assert_eq!(r109_count(&driver, "Eldrazi Horror"), 0);
    r109_replays(&driver);
    Ok(())
}
