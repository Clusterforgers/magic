//! Goal: a mana ability granted by a static ability works for exactly the permanents the static names, at two, three and four seats: lands of the controller (Chromatic Lantern) produce any color, a creature of the controller (Citanul Hierophants) taps for green unless it is summoning sick, every Sliver of either side (Gemhide Sliver) produces any color, the grant ends with its source, and the permanents of another player stay plain; the mana pays for a spell.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::mana::ManaType;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, instant, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick};
use rstest::rstest;

use mtg_core::color::Color;

/// Seat `active` is in its first main phase; the others hold nothing.
fn main_phase(seats: usize, active: usize) -> Scenario {
    Scenario::new(seats).at(3, active, Step::Main1)
}

fn tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .is_some_and(mtg_engine::object::PermanentState::tapped)
}

#[rstest]
#[case(ManaType::White)]
#[case(ManaType::Blue)]
#[case(ManaType::Black)]
#[case(ManaType::Red)]
#[case(ManaType::Green)]
fn chromatic_lantern_lets_a_land_make_any_color(
    #[values(2, 3, 4)] seats: usize,
    #[case] mana: ManaType,
) -> Result<(), ScriptError> {
    let scenario = main_phase(seats, 0).battlefield(
        0,
        [ready(card("Chromatic Lantern").unwrap()), ready(mountain())],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.tap_for_mana_of("Mountain", mana)?;
    assert!(tapped(&driver, "Mountain"));
    Ok(())
}

#[rstest]
fn a_land_makes_only_its_own_color_without_the_lantern(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = main_phase(seats, 0).battlefield(0, [ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(driver.tap_for_mana_of("Mountain", ManaType::White).is_err());
    driver.tap_for_mana_of("Mountain", ManaType::Red)?;
    Ok(())
}

#[rstest]
fn the_lanterns_grant_is_for_the_lands_of_its_controller(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // It is seat 1's turn; the Lantern and a Mountain are seat 0's, seat 1's Mountain is plain.
    let scenario = main_phase(seats, 1)
        .battlefield(0, [ready(card("Chromatic Lantern").unwrap())])
        .battlefield(1, [ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(driver.tap_for_mana_of("Mountain", ManaType::White).is_err());
    driver.tap_for_mana_of("Mountain", ManaType::Red)?;
    Ok(())
}

#[rstest]
fn the_grant_pays_for_a_spell(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = main_phase(seats, 0)
        .battlefield(
            0,
            [ready(card("Chromatic Lantern").unwrap()), ready(mountain())],
        )
        .hand(0, [instant("Smite", 0, &[Color::White])]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.tap_for_mana_of("Mountain", ManaType::White)?;
    driver.cast("Smite")?;
    assert!(tapped(&driver, "Mountain"));
    Ok(())
}

#[rstest]
#[case::ready(false, true)]
#[case::summoning_sick(true, false)]
fn citanul_hierophants_gives_each_creature_of_its_controller_a_green_tap(
    #[values(2, 3, 4)] seats: usize,
    #[case] is_sick: bool,
    #[case] taps: bool,
) -> Result<(), ScriptError> {
    let bear = free_creature("Bear", 2, 2);
    let bear = if is_sick { sick(bear) } else { ready(bear) };
    let scenario =
        main_phase(seats, 0).battlefield(0, [ready(card("Citanul Hierophants").unwrap()), bear]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(
        driver.tap_for_mana_of("Bear", ManaType::Green).is_ok(),
        taps
    );
    assert_eq!(tapped(&driver, "Bear"), taps);
    // The Hierophants is a creature too (ready), and makes only green.
    assert!(
        driver
            .tap_for_mana_of("Citanul Hierophants", ManaType::Red)
            .is_err()
    );
    Ok(())
}

#[rstest]
fn the_hierophants_grant_ends_with_it_and_skips_the_creatures_of_others(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Seat 1's Bear has no Hierophants of its own on its turn.
    let scenario = main_phase(seats, 1)
        .battlefield(0, [ready(card("Citanul Hierophants").unwrap())])
        .battlefield(1, [ready(free_creature("Bear", 2, 2))]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(driver.tap_for_mana_of("Bear", ManaType::Green).is_err());
    Ok(())
}

#[rstest]
fn gemhide_sliver_gives_every_sliver_a_tap_for_any_color(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] owner: usize,
    #[values(
        ManaType::White,
        ManaType::Blue,
        ManaType::Black,
        ManaType::Red,
        ManaType::Green
    )]
    mana: ManaType,
) -> Result<(), ScriptError> {
    // The Sliver of seat `owner` is tapped by its controller, who is the active player.
    let scenario =
        main_phase(seats, owner).battlefield(owner, [ready(card("Gemhide Sliver").unwrap())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.tap_for_mana_of("Gemhide Sliver", mana)?;
    assert!(tapped(&driver, "Gemhide Sliver"));
    Ok(())
}

#[rstest]
fn a_sliver_of_another_player_is_given_the_ability_by_a_gemhide_sliver_of_any_player(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // "All Slivers": seat 1's Manaweft Sliver taps for any color with seat 0's Gemhide Sliver on the battlefield too.
    let scenario = main_phase(seats, 1)
        .battlefield(0, [ready(card("Gemhide Sliver").unwrap())])
        .battlefield(1, [ready(card("Manaweft Sliver").unwrap())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.tap_for_mana_of("Manaweft Sliver", ManaType::Blue)?;
    Ok(())
}
