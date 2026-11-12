//! Goal: a split card with fuse in a hand may be cast as both halves for the sum of their costs, listed beside its halves in the options when both can be cast (an artifact and an enchantment to destroy, {1}{R}{W} to pay) and only then; the fused spell is the card as a whole on the stack (its two names, three mana value, red and white), asks the targets of the left half and then those of the right, and resolves the left half and then the right, destroying both; a half cast on its own is that half alone; the card is the whole card again in the graveyard; and a fuse card outside a hand is never fused. At two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_engine::{FaceOption, PresetPermanent};
use mtg_testkit::cards::{mountain, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, pay};
use crate::common::{name_of, names, pid};

const CARD: &str = "Wear // Tear";

fn lands(red: usize, white: usize) -> Vec<PresetPermanent> {
    (0..red)
        .map(|_| ready(mountain()))
        .chain((0..white).map(|_| ready(plains())))
        .collect()
}

/// Seat 0 holds the card with the lands; the last seat controls the artifact and the enchantment, if any.
fn table(
    seats: usize,
    red: usize,
    white: usize,
    artifact: bool,
    enchantment: bool,
) -> Result<Driver, ScriptError> {
    let mut theirs = Vec::new();
    if artifact {
        theirs.push("Sol Ring");
    }
    if enchantment {
        theirs.push("Glorious Anthem");
    }
    Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, [CARD])
            .battlefield(0, lands(red, white))
            .battlefield_named(seats - 1, theirs),
    )
}

fn listed(driver: &Driver) -> Vec<(String, u8)> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority is asked");
    };
    options
        .faces
        .iter()
        .map(|option: &FaceOption| (option.name.clone(), option.face))
        .collect()
}

#[rstest]
#[case::both_targets_and_the_mana(2, 1, 2, true, true, &["Wear", "Tear", "Wear // Tear"])]
#[case::one_mana_short(2, 1, 1, true, true, &["Wear", "Tear"])]
#[case::no_enchantment_to_destroy(2, 1, 2, true, false, &["Wear"])]
#[case::no_artifact_to_destroy(3, 1, 2, false, true, &["Tear"])]
#[case::no_target_at_all(4, 1, 2, false, false, &[])]
#[case::only_the_right_half_is_payable(2, 0, 1, true, true, &["Tear"])]
fn the_card_as_a_whole_is_listed_when_both_halves_can_be_cast(
    #[case] seats: usize,
    #[case] red: usize,
    #[case] white: usize,
    #[case] artifact: bool,
    #[case] enchantment: bool,
    #[case] want: &[&str],
) -> Result<(), ScriptError> {
    let driver = table(seats, red, white, artifact, enchantment)?;
    let got = listed(&driver);
    assert_eq!(
        got.iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        want
    );
    if want.contains(&CARD) {
        assert_eq!(
            got.last().unwrap().1,
            2,
            "the whole card is the position after the last half"
        );
    }
    Ok(())
}

#[rstest]
fn both_halves_are_cast_together_and_resolve_left_then_right(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, 2, 2, true, true)?;
    driver.begin_cast(CARD)?;
    choose_way(&mut driver, CARD)?;
    // The artifact of the left half is chosen, then the enchantment of the right.
    let artifact = driver.target_named("Sol Ring")?;
    driver.choose_targets(&[artifact])?;
    let enchantment = driver.target_named("Glorious Anthem")?;
    driver.choose_targets(&[enchantment])?;
    pay(&mut driver)?;
    let spell = *driver.state().zone(Zone::Stack).last().unwrap();
    let found = characteristics(driver.state(), spell).unwrap();
    assert_eq!(name_of(driver.state(), spell), CARD);
    assert_eq!(driver.state().object(spell).unwrap().card().mana_value(), 3);
    assert_eq!(
        found.colors.iter().collect::<Vec<_>>(),
        [Color::White, Color::Red]
    );
    assert_eq!(driver.state().object(spell).unwrap().face(), Some(2));
    for _ in 0..seats {
        driver.pass()?;
    }
    assert_eq!(
        names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|n| *n != "Mountain" && *n != "Plains")
            .count(),
        0
    );
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), [CARD]);
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(seats - 1))).len(),
        2
    );
    Ok(())
}

#[rstest]
#[case::the_left_half("Wear", "Sol Ring", 2)]
#[case::the_right_half("Tear", "Glorious Anthem", 1)]
fn a_half_cast_alone_is_only_that_half(
    #[case] half: &str,
    #[case] target: &str,
    #[case] value: u32,
) -> Result<(), ScriptError> {
    let mut driver = table(2, 2, 2, true, true)?;
    driver.begin_cast(CARD)?;
    choose_way(&mut driver, half)?;
    let target = driver.target_named(target)?;
    driver.choose_targets(&[target])?;
    pay(&mut driver)?;
    let spell = *driver.state().zone(Zone::Stack).last().unwrap();
    assert_eq!(name_of(driver.state(), spell), half);
    assert_eq!(
        driver.state().object(spell).unwrap().card().mana_value(),
        value
    );
    driver.pass()?;
    driver.pass()?;
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(1))).len(), 1);
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), [CARD]);
    Ok(())
}

#[test]
fn a_fuse_card_that_is_not_in_a_hand_is_not_fused() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .graveyard_named(0, [CARD])
            .battlefield(0, lands(2, 2))
            .battlefield_named(1, ["Sol Ring", "Glorious Anthem"]),
    )?;
    assert_eq!(listed(&driver), []);
    Ok(())
}
