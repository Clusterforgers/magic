//! Goal: mana and amounts that read a tally of the turn (lands played, cards drawn) or the lands a player controls (River of Tears, Kydele, the three Urza lands, Knowledge Is Power, Fists of Flame) are worked out when the ability is offered or the effect resolves, reset with the turn, and replay the same at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_core::mana::ManaType;
use mtg_engine::characteristics::characteristics;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, free_creature, island, mountain, study};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 controls these cards and holds these in hand; it is turn 1, precombat main phase, and every seat has a library.
fn table(
    seats: usize,
    battlefield: &[&str],
    hand: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(0, battlefield.iter().map(|name| ready(card(name).unwrap())));
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..12).map(|_| forest()));
    }
    Driver::from_scenario(scenario)
}

/// The mana of seat 0, by type.
fn pool(driver: &Driver, mana: ManaType) -> u32 {
    driver.state().player(pid(0)).unwrap().mana().get(mana)
}

/// Passes until nothing is on the stack.
fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    panic!("the stack did not resolve");
}

#[rstest]
#[case::before_a_land(false, ManaType::Blue)]
#[case::after_a_land(true, ManaType::Black)]
fn river_of_tears_adds_black_once_a_land_was_played_this_turn(
    #[case] played: bool,
    #[case] adds: ManaType,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, &["River of Tears"], vec![forest()])?;
    if played {
        driver.play_land("Forest")?;
    }
    driver.tap_for_mana("River of Tears")?;
    assert_eq!(pool(&driver, adds), 1);
    assert_eq!(
        driver.state().player(pid(0)).unwrap().mana().total(),
        1,
        "one mana, of the other type only"
    );
    Ok(())
}

#[rstest]
fn the_land_tally_resets_with_the_turn_and_counts_only_the_player_who_played(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, &["River of Tears"], vec![forest()])?;
    driver.play_land("Forest")?;
    driver.idle_until(u32::try_from(seats).unwrap() + 1, Step::Main1)?;
    driver.tap_for_mana("River of Tears")?;
    assert_eq!(pool(&driver, ManaType::Blue), 1, "a new turn: no land yet");
    Ok(())
}

#[rstest]
fn kydele_counts_the_cards_drawn_by_a_spell_and_resets_with_the_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [study("Study")])
        .battlefield(
            0,
            [
                ready(card("Kydele, Chosen of Kruphix").unwrap()),
                ready(island()),
            ],
        );
    let mut scenario = scenario;
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..12).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(driver.tap_for_mana("Kydele, Chosen of Kruphix").is_err());
    driver.cast("Study")?;
    resolve(&mut driver)?;
    driver.tap_for_mana("Kydele, Chosen of Kruphix")?;
    assert_eq!(pool(&driver, ManaType::Colorless), 2, "two cards drawn");
    driver.idle_until(u32::try_from(seats).unwrap() + 1, Step::Main1)?;
    driver.tap_for_mana("Kydele, Chosen of Kruphix")?;
    assert_eq!(
        pool(&driver, ManaType::Colorless),
        1,
        "the draw step of the new turn"
    );
    Ok(())
}

#[rstest]
#[case::none(&[], 1)]
#[case::the_tower_only(&["Urza's Tower"], 1)]
#[case::the_power_plant_only(&["Urza's Power Plant"], 1)]
#[case::both(&["Urza's Tower", "Urza's Power Plant"], 2)]
fn urzas_mine_adds_two_with_the_other_two_urza_lands(
    #[case] others: &[&str],
    #[case] adds: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut permanents = vec!["Urza's Mine"];
    permanents.extend(others);
    let mut driver = table(seats, &permanents, Vec::new())?;
    driver.tap_for_mana("Urza's Mine")?;
    assert_eq!(pool(&driver, ManaType::Colorless), adds);
    Ok(())
}

#[rstest]
#[case::power_plant("Urza's Power Plant", &["Urza's Mine", "Urza's Tower"])]
#[case::tower("Urza's Tower", &["Urza's Mine", "Urza's Power Plant"])]
fn each_urza_land_counts_the_other_two_it_controls_and_not_an_opponents(
    #[case] tapped: &str,
    #[case] others: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut with_all = vec![tapped];
    with_all.extend(others);
    let mut driver = table(seats, &with_all, Vec::new())?;
    driver.tap_for_mana(tapped)?;
    let expected = if tapped == "Urza's Tower" { 3 } else { 2 };
    assert_eq!(pool(&driver, ManaType::Colorless), expected);
    // The other two under an opponent's control do not count.
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card(tapped).unwrap())])
        .battlefield(1, others.iter().map(|name| ready(card(name).unwrap())));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.tap_for_mana(tapped)?;
    assert_eq!(pool(&driver, ManaType::Colorless), 1);
    Ok(())
}

/// The power of a creature.
fn power(driver: &Driver, name: &str) -> i32 {
    let id = driver.id_of(name).unwrap();
    characteristics(driver.state(), id).unwrap().power.unwrap()
}

#[rstest]
fn knowledge_is_power_grows_your_creatures_by_the_cards_you_drew(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [study("Study")])
        .battlefield(
            0,
            [
                ready(card("Knowledge Is Power").unwrap()),
                ready(free_creature("Mine", 1, 1)),
                ready(island()),
            ],
        )
        .battlefield(1, [ready(free_creature("Theirs", 1, 1))]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..12).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(power(&driver, "Mine"), 1);
    driver.cast("Study")?;
    resolve(&mut driver)?;
    assert_eq!(power(&driver, "Mine"), 3, "two cards drawn");
    assert_eq!(power(&driver, "Theirs"), 1, "only your creatures");
    driver.idle_until(u32::try_from(seats).unwrap() + 1, Step::Main1)?;
    assert_eq!(power(&driver, "Mine"), 2, "the draw step of the new turn");
    Ok(())
}

#[rstest]
#[case::alone(false, 1)]
#[case::after_a_cantrip(true, 3)]
fn fists_of_flame_gives_a_bonus_for_each_card_drawn_this_turn(
    #[case] study_first: bool,
    #[case] bonus: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [study("Study"), card("Fists of Flame").unwrap()])
        .battlefield(
            0,
            [
                ready(free_creature("Mine", 1, 1)),
                ready(island()),
                ready(mountain()),
                ready(mountain()),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..12).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    if study_first {
        driver.cast("Study")?;
        resolve(&mut driver)?;
    }
    let target = driver.target_named("Mine")?;
    driver.cast_at("Fists of Flame", &[target])?;
    resolve(&mut driver)?;
    assert_eq!(power(&driver, "Mine"), 1 + bonus);
    Ok(())
}
