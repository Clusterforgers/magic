//! Goal: an effect that sets creature types ("becomes a red Weird", CR 205.1b, layer 4) replaces the printed creature types and leaves the others, and Turn // Burn does all four things its Turn half says to the same creature until end of turn (loses its abilities, becomes a red Weird, 0/1 by layer 7b) while later effects of layer 7c still add: tables at two to four seats, with the fuse cast of both halves and the characteristics back at the next turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::Subtype;
use mtg_core::color::Color;
use mtg_engine::characteristics::characteristics;
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, pay};
use crate::common::removal::resolve_the_stack;

struct Seen {
    power: Option<i32>,
    toughness: Option<i32>,
    colors: Vec<Color>,
    types: Vec<String>,
    flying: bool,
}

fn seen(driver: &Driver, name: &str) -> Seen {
    let id = driver.id_of(name).unwrap();
    let chars = characteristics(driver.state(), id).unwrap();
    let printed = &driver.state().object(id).unwrap().card().subtypes;
    let types = ["Angel", "Elf", "Druid", "Weird", "Bear"]
        .into_iter()
        .filter(|name| chars.has_subtype(printed, &Subtype((*name).to_owned())))
        .map(str::to_owned)
        .collect();
    let flying = keywords_of(driver.state(), id)
        .iter()
        .any(|keyword| format!("{keyword:?}").contains("Flying"));
    Seen {
        power: chars.power,
        toughness: chars.toughness,
        colors: chars.colors.iter().collect(),
        types,
        flying,
    }
}

#[rstest]
#[case::an_angel("Serra Angel", &[], (4, 4), &["Angel"], true, (0, 1))]
#[case::an_elf_druid("Llanowar Elves", &[], (1, 1), &["Elf", "Druid"], false, (0, 1))]
#[case::a_bear("Grizzly Bears", &[], (2, 2), &["Bear"], false, (0, 1))]
#[case::later_effects_still_add("Grizzly Bears", &["Glorious Anthem"], (3, 3), &["Bear"], false, (1, 2))]
fn turn_makes_a_creature_a_red_weird_until_end_of_turn(
    #[values(2, 3, 4)] seats: usize,
    #[case] creature: &str,
    #[case] others: &[&str],
    #[case] before: (i32, i32),
    #[case] types_before: &[&str],
    #[case] flying_before: bool,
    #[case] after: (i32, i32),
) -> Result<(), ScriptError> {
    let mut theirs = vec![creature];
    theirs.extend(others);
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Turn // Burn"])
            .battlefield(
                0,
                [island(), island(), island()]
                    .into_iter()
                    .chain([mountain(), mountain()])
                    .map(ready),
            )
            .battlefield_named(1, theirs),
    )?;
    let first = seen(&driver, creature);
    assert_eq!(
        (first.power, first.toughness),
        (Some(before.0), Some(before.1))
    );
    assert_eq!(first.types, types_before);
    assert_eq!(first.flying, flying_before);
    driver.begin_cast("Turn // Burn")?;
    choose_way(&mut driver, "Turn")?;
    let target = driver.target_named(creature)?;
    driver.choose_targets(&[target])?;
    pay(&mut driver)?;
    resolve_the_stack(&mut driver)?;
    let weird = seen(&driver, creature);
    assert_eq!(
        (weird.power, weird.toughness),
        (Some(after.0), Some(after.1))
    );
    assert_eq!(weird.types, ["Weird"]);
    assert_eq!(weird.colors, [Color::Red]);
    assert!(!weird.flying);
    // The effect ends with the turn.
    driver.idle_until(2, Step::Upkeep)?;
    let back = seen(&driver, creature);
    assert_eq!(
        (back.power, back.toughness),
        (Some(before.0), Some(before.1))
    );
    assert_eq!(back.types, types_before);
    assert_eq!(back.flying, flying_before);
    Ok(())
}
