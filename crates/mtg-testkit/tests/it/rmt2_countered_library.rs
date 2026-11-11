//! Goal: a spell countered by Memory Lapse or Spell Crumple goes on top of or on the bottom of its owner's library instead of into the graveyard, Spell Crumple then goes on the bottom of its own owner's library, and a spell that can't be countered resolves and goes to the graveyard as usual, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::removal::spell_named;
use common::{pid, spell};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::island;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An instant that does nothing, which can't be countered when `sturdy`.
fn victim(sturdy: bool) -> Arc<CardDefinition> {
    let card = spell("Victim", CardType::Instant, "[]", "[]");
    if !sturdy {
        return card;
    }
    let rule: Ability =
        serde_json::from_str(r#"{"Rule":{"from":"Stack","rule":{"CantBeCountered":"This"}}}"#)
            .unwrap();
    Arc::new((*card).clone().with_rules(rule))
}

fn pass_until_priority(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    for _ in 0..16 {
        if driver.decision()?.player == pid(seat) {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck(format!("priority for seat {seat}")))
}

/// The names of a player's library, top card first.
fn library(driver: &Driver, seat: usize) -> Vec<String> {
    driver
        .state()
        .zone(Zone::Library(pid(seat)))
        .iter()
        .rev()
        .map(|id| {
            driver
                .state()
                .object(*id)
                .map_or_else(String::new, |o| o.card().name.clone())
        })
        .collect()
}

#[rstest]
#[case::lapse_puts_it_on_top("Memory Lapse", false, Some(true))]
#[case::crumple_puts_it_on_the_bottom("Spell Crumple", false, Some(false))]
#[case::lapse_cant_counter_a_sturdy_spell("Memory Lapse", true, None)]
#[case::crumple_cant_counter_a_sturdy_spell("Spell Crumple", true, None)]
fn a_countered_spell_goes_into_its_owners_library(
    #[case] counter: &str,
    #[case] sturdy: bool,
    #[case] on_top: Option<bool>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [victim(sturdy)])
            .library_named(0, ["Grizzly Bears", "Grizzly Bears"])
            .library_named(1, ["Grizzly Bears", "Grizzly Bears"])
            .hand_named(1, [counter])
            .battlefield(1, [ready(island()), ready(island()), ready(island())]),
    )?;
    driver.cast("Victim")?;
    pass_until_priority(&mut driver, 1)?;
    let target = spell_named(&driver, "Victim");
    driver.cast_at(counter, &[target])?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let owner = library(&driver, 0);
    match on_top {
        Some(true) => assert_eq!(owner.first().map(String::as_str), Some("Victim")),
        Some(false) => assert_eq!(owner.last().map(String::as_str), Some("Victim")),
        None => {
            assert!(!owner.iter().any(|name| name == "Victim"));
            driver.expect_zone(Zone::Graveyard(pid(0)), &["Victim"])?;
        }
    }
    if on_top.is_some() {
        driver.expect_zone(Zone::Graveyard(pid(0)), &[])?;
    }
    let caster = library(&driver, 1);
    if counter == "Spell Crumple" {
        assert_eq!(caster.last().map(String::as_str), Some("Spell Crumple"));
        driver.expect_zone(Zone::Graveyard(pid(1)), &[])
    } else {
        assert!(!caster.iter().any(|name| name == counter));
        driver.expect_zone(Zone::Graveyard(pid(1)), &[counter])
    }
}
