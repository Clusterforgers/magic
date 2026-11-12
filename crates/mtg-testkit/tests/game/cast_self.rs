//! Goal: a cast trigger of the card itself ("When you cast this spell, copy it for each time you've cast your commander from the command zone this game") works from the stack at 2 to 4 seats after 0, 1, 2 and 3 casts of the commander: the trigger goes on the stack above the spell, the copies are put on the stack as it resolves, a copy is not cast and has no trigger of its own, a sorcery, a creature spell and a spell with a target each leave one result for the spell and one for each copy, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::format::Commander;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{island, mountain, plains};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::commander_casts::{cast_times, resolve};
use crate::common::copying::{casts, keep_targets_until_done, replays, spell_copies};

const SEAT: PlayerId = PlayerId(0);

/// A Commander table where seat 0 has a red commander in the command zone, the corpus cards `hand` in its hand and lands
/// for the commander's casts (two Mountains, and two more for the tax of each earlier cast) and for the cards.
fn table(seats: usize, hand: &[&str]) -> Scenario {
    let lands = [plains(), plains(), island(), island()]
        .into_iter()
        .chain((0..24).map(|_| mountain()))
        .map(ready)
        .chain([ready(corpus::card("Sol Ring").unwrap())]);
    Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, vec![legend("Cmd", 1, &[Color::Red], 2, 2)])
        .battlefield(0, lands)
        .hand_named(0, hand.iter().copied())
}

/// The permanents of seat 0 named `name`, and how many of them are tokens.
fn permanents(driver: &Driver, name: &str) -> (usize, usize) {
    let state = driver.state();
    let found: Vec<_> = state
        .zone(Zone::Battlefield)
        .iter()
        .map(|id| state.object(*id).unwrap())
        .filter(|object| object.card().name == name && object.controller() == SEAT)
        .collect();
    (
        found.len(),
        found
            .iter()
            .filter(|object| object.kind() == ObjectKind::Token)
            .count(),
    )
}

/// Casts the card `name` for `taps` and checks the trigger of the card itself on the stack above it, with no copy yet.
fn cast_and_watch(driver: &mut Driver, name: &str, taps: &[&str]) -> Result<(), ScriptError> {
    let before = casts(driver);
    driver.cast_tapping(name, taps)?;
    assert_eq!(casts(driver), before + 1);
    assert_eq!(driver.state().stack().len(), 2, "the spell and its trigger");
    assert_eq!(spell_copies(driver).len(), 0, "no copy yet");
    Ok(())
}

#[rstest]
fn a_sorcery_makes_its_result_for_the_spell_and_for_each_commander_cast(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] earlier: u32,
) -> Result<(), ScriptError> {
    let setup = table(seats, &["Empyrial Storm"]).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &["Empyrial Storm"]))?;
    cast_times(&mut driver, "Cmd", earlier)?;
    let cast_before = casts(&driver);
    cast_and_watch(
        &mut driver,
        "Empyrial Storm",
        &[
            "Plains", "Plains", "Mountain", "Mountain", "Mountain", "Mountain",
        ],
    )?;
    resolve(&mut driver)?;
    assert_eq!(
        permanents(&driver, "Angel"),
        (1 + earlier as usize, 1 + earlier as usize)
    );
    assert_eq!(casts(&driver), cast_before + 1, "a copy is not cast");
    assert_eq!(driver.state().commander_casts_by(SEAT), earlier);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn the_copies_are_on_the_stack_when_the_trigger_has_resolved(
    #[values(2, 4)] seats: usize,
    #[values(0, 2, 3)] earlier: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &["Empyrial Storm"]))?;
    cast_times(&mut driver, "Cmd", earlier)?;
    cast_and_watch(
        &mut driver,
        "Empyrial Storm",
        &[
            "Plains", "Plains", "Mountain", "Mountain", "Mountain", "Mountain",
        ],
    )?;
    // Every seat passes once: the trigger resolves and the copies lie above the spell.
    for _ in 0..seats {
        driver.pass()?;
    }
    assert_eq!(spell_copies(&driver).len(), earlier as usize);
    assert_eq!(driver.state().stack().len(), 1 + earlier as usize);
    Ok(())
}

#[rstest]
fn a_creature_spell_leaves_a_token_copy_for_each_commander_cast(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 3)] earlier: u32,
) -> Result<(), ScriptError> {
    let setup = table(seats, &["Hatut Zeraze Strike Force"])
        .setup()
        .unwrap();
    let mut driver = Driver::from_scenario(table(seats, &["Hatut Zeraze Strike Force"]))?;
    cast_times(&mut driver, "Cmd", earlier)?;
    cast_and_watch(
        &mut driver,
        "Hatut Zeraze Strike Force",
        &["Plains", "Mountain", "Mountain", "Mountain"],
    )?;
    keep_targets_until_done(&mut driver, seats)?;
    // The card, and a token copy for each cast; no copy copies itself again.
    assert_eq!(
        permanents(&driver, "Hatut Zeraze Strike Force"),
        (1 + earlier as usize, earlier as usize)
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_spell_with_a_target_is_copied_with_the_same_target(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] earlier: u32,
) -> Result<(), ScriptError> {
    let setup = table(seats, &["Echo Storm"]).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &["Echo Storm"]))?;
    cast_times(&mut driver, "Cmd", earlier)?;
    let ring = driver.target_named("Sol Ring")?;
    driver.begin_cast("Echo Storm")?;
    driver.choose_targets(&[ring])?;
    driver.pay_tapping(&["Island", "Island", "Mountain", "Mountain", "Mountain"], 0)?;
    assert_eq!(driver.state().stack().len(), 2, "the spell and its trigger");
    keep_targets_until_done(&mut driver, seats)?;
    // Sol Ring, and a token copy of it for the spell and for each copy.
    assert_eq!(
        permanents(&driver, "Sol Ring"),
        (2 + earlier as usize, 1 + earlier as usize)
    );
    replays(&driver, &setup);
    Ok(())
}
