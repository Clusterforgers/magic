//! Goal: a token copy keeps the exceptions that add to it (CR 707.9b), at 2 to 4 seats: "a 2/2 black Zombie in addition to its other colors and types" keeps the original's color and creature types beside the new ones, and "it's not legendary and its starting loyalty is N" makes a planeswalker that is no legend and enters with N loyalty counters; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::Color;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_testkit::commander_cards::legend;
use mtg_testkit::loyalty_cards::dawn_walker;
use mtg_testkit::trigger_cards::sized_creature_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::common::copying::{replays, run};

/// A 1/1 with "{0}: Create a token that's a copy of target `kind`, except `except`."
fn maker(kind: &str, except: &str) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Activated":{{"cost":[],"targets":[{{"what":{{"Object":[{{"Type":"{kind}"}}]}}}}],"effects":[{{"CreateToken":{{"token":{{"CopyOf":{{"of":{{"Target":0}},"except":{except}}}}}}}}}]}}}}"#
    );
    sized_creature_with(
        "Maker",
        1,
        1,
        vec![serde_json::from_str::<Ability>(&json).unwrap()],
    )
}

/// Makes the copy of `copied` and gives the token that comes of it.
fn copy_made(
    seats: usize,
    kind: &str,
    except: &str,
    copied: Arc<CardDefinition>,
) -> Result<(Driver, mtg_core::ids::ObjectId), ScriptError> {
    let name = copied.name.clone();
    let loyalty = copied.loyalty.unwrap_or(0);
    let scenario = || {
        Scenario::new(seats).battlefield(
            0,
            vec![
                ready(maker(kind, except)),
                with_counters(ready(copied.clone()), CounterKind::Loyalty, loyalty),
            ],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_activation("Maker")?;
    let target = driver.target_named(&name)?;
    driver.choose_targets(&[target])?;
    run(&mut driver, seats)?;
    replays(&driver, &setup);
    let state = driver.state();
    let token = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            state
                .object(*id)
                .is_some_and(|object| object.kind() == ObjectKind::Token)
        })
        .expect("a token copy");
    Ok((driver, token))
}

#[rstest]
#[case::one_color(r#"["Black"]"#, &[Color::Blue, Color::Black])]
#[case::two_colors(r#"["Black","Green"]"#, &[Color::Blue, Color::Black, Color::Green])]
#[case::the_same_color(r#"["Blue"]"#, &[Color::Blue])]
fn colors_and_types_in_addition_keep_the_originals(
    #[values(2, 3, 4)] seats: usize,
    #[case] colors: &str,
    #[case] expected: &[Color],
) -> Result<(), ScriptError> {
    let except = format!(
        r#"[{{"RemoveSupertype":"Legendary"}},{{"SetPT":{{"power":2,"toughness":2}}}},{{"AddColors":{colors}}},{{"AddSubtype":"Zombie"}}]"#
    );
    let hero = legend("Hero", 1, &[Color::Blue], 5, 5);
    let (driver, token) = copy_made(seats, "Creature", &except, hero)?;
    let copy = driver.state().object(token).unwrap().card();
    assert_eq!(copy.name, "Hero");
    assert_eq!((copy.power, copy.toughness), (Some(2), Some(2)));
    assert!(!copy.supertypes.contains(&Supertype::Legendary));
    assert!(copy.subtypes.contains(&Subtype("Zombie".to_owned())));
    let colors = copy.colors();
    for color in [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ] {
        assert_eq!(
            colors.contains(color),
            expected.contains(&color),
            "{color:?}"
        );
    }
    Ok(())
}

#[rstest]
#[case::one(1)]
#[case::three(3)]
fn a_starting_loyalty_sets_the_counters_the_copy_enters_with(
    #[values(2, 3, 4)] seats: usize,
    #[case] loyalty: i32,
) -> Result<(), ScriptError> {
    let except = format!(r#"[{{"RemoveSupertype":"Legendary"}},{{"SetLoyalty":{loyalty}}}]"#);
    let (driver, token) = copy_made(seats, "Planeswalker", &except, dawn_walker().unwrap())?;
    let object = driver.state().object(token).unwrap();
    assert!(object.card().types.contains(&CardType::Planeswalker));
    assert!(!object.card().supertypes.contains(&Supertype::Legendary));
    let counters = object
        .permanent()
        .map(|permanent| permanent.counters_of(&CounterKind::Loyalty));
    assert_eq!(counters, Some(u32::try_from(loyalty).unwrap()));
    Ok(())
}
