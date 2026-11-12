//! Goal: the copies that end with the turn (Hate Mirage: tokens with haste that are exiled at the next end step, also when the creature they copy has left), the creatures that died under a player's control this turn (counted for the controller and not for an opponent; Gorma, the Gullet puts a counter on the nontoken creatures that enter after them), at tables of two, three and four seats, replayed from the log.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics;
use mtg_engine::keywords::keywords_of;
use mtg_engine::object::ObjectKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::{replays, settle};

/// Seat 0 holds `hand` over eight Mountains and controls Gorma and a bear; seat 1 controls two creatures; every other
/// seat controls a guard.
fn table(seats: usize, hand: &[&str]) -> Scenario {
    let mut ours: Vec<_> = (0..8).map(|_| ready(mountain())).collect();
    ours.push(ready(card("Gorma, the Gullet").unwrap()));
    ours.push(ready(creature("Own Bear", 1, &[Color::Red], 1, 1)));
    let mut scenario = Scenario::new(seats)
        .hand(0, hand.iter().map(|name| card(name).unwrap()))
        .battlefield(0, ours)
        .battlefield(
            1,
            [
                ready(creature("Opposing Giant", 1, &[Color::Red], 3, 3)),
                ready(creature("Opposing Guard", 1, &[Color::Red], 1, 1)),
            ],
        );
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(creature("Guard", 1, &[Color::Red], 2, 2))]);
    }
    scenario
}

/// The permanents of the battlefield named `name`, in id order.
fn named(driver: &Driver, name: &str) -> Vec<ObjectId> {
    let mut found: Vec<ObjectId> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().card().name == name)
        .collect();
    found.sort();
    found
}

/// Casts `spell` with its targets and lets it resolve.
fn cast(
    driver: &mut Driver,
    spell: &str,
    targets: &[&str],
    taps: usize,
) -> Result<(), ScriptError> {
    let chosen: Vec<_> = targets
        .iter()
        .map(|name| driver.target_named(name))
        .collect::<Result<_, _>>()?;
    driver.begin_cast(spell)?;
    if !chosen.is_empty() {
        driver.choose_targets(&chosen)?;
    }
    driver.pay_tapping(&vec!["Mountain"; taps], 0)?;
    settle(driver)
}

#[rstest]
fn hate_mirage_copies_two_creatures_and_exiles_the_copies_at_the_end_step_even_when_the_originals_left(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hand = ["Hate Mirage", "Lightning Bolt"];
    let setup = table(seats, &hand).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &hand))?;
    cast(
        &mut driver,
        "Hate Mirage",
        &["Opposing Giant", "Opposing Guard"],
        4,
    )?;
    for name in ["Opposing Giant", "Opposing Guard"] {
        let both = named(&driver, name);
        assert_eq!(both.len(), 2, "{name} and its copy");
        let token = both
            .into_iter()
            .find(|id| driver.state().object(*id).unwrap().kind() == ObjectKind::Token)
            .unwrap();
        assert!(
            keywords_of(driver.state(), token)
                .iter()
                .any(|keyword| format!("{keyword:?}").contains("Haste")),
            "{name} copy has haste"
        );
        assert_eq!(
            driver.state().object(token).unwrap().controller().index(),
            0
        );
    }
    // The creature a copy was made of leaves; its copy stays until the end step.
    cast(&mut driver, "Lightning Bolt", &["Opposing Guard"], 1)?;
    let tokens = |driver: &Driver| {
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| driver.state().object(**id).unwrap().kind() == ObjectKind::Token)
            .count()
    };
    assert!(tokens(&driver) >= 2, "the copies stay until the end step");
    replays(&driver, &setup);
    driver.idle_until(1, Step::Main2)?;
    assert!(tokens(&driver) >= 2, "the copies stay until the end step");
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(tokens(&driver), 0, "the end step exiled every copy");
    assert_eq!(named(&driver, "Opposing Giant").len(), 1);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn gorma_counts_the_creatures_that_died_under_its_controller_and_not_under_an_opponent(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] ours: bool,
) -> Result<(), ScriptError> {
    let newcomer = creature("Newcomer", 1, &[Color::Red], 1, 1);
    let scenario = |seats| {
        table(seats, &["Lightning Bolt"])
            .hand(0, [card("Lightning Bolt").unwrap(), newcomer.clone()])
    };
    let setup = scenario(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(scenario(seats))?;
    let dead = if ours { "Own Bear" } else { "Opposing Guard" };
    cast(&mut driver, "Lightning Bolt", &[dead], 1)?;
    cast(&mut driver, "Newcomer", &[], 2)?;
    let entered = driver.id_of("Newcomer")?;
    let power = characteristics(driver.state(), entered).unwrap().power;
    // Only a creature that died under the control of Gorma's controller adds a counter.
    assert_eq!(power, Some(if ours { 2 } else { 1 }));
    assert!(
        characteristics(driver.state(), entered)
            .unwrap()
            .types
            .contains(CardType::Creature)
    );
    replays(&driver, &setup);
    Ok(())
}
