//! Goal: a copy "except it's a 1/1 green Frog" has the size, the color and the creature types the exception names and no other creature type, while the layers apply on top; at tables of two to four seats, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::{replays, run};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::Subtype;
use mtg_engine::choose::Target;
use mtg_testkit::corpus::card;
use mtg_testkit::trigger_cards::sized_creature_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

/// A 1/1 with "{0}: This becomes a copy of another target creature, except `except`."
fn g12b_shifter(except: &str) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Activated":{{"cost":[],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},"Other"]}}}}],"effects":[{{"BecomeCopy":{{"what":"This","of":{{"Target":0}},"except":{except},"duration":"Forever"}}}}]}}}}"#
    );
    sized_creature_with(
        "Shifter",
        1,
        1,
        vec![serde_json::from_str::<Ability>(&json).unwrap()],
    )
}

#[rstest]
fn g12b_a_copy_is_only_the_creature_types_it_names(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        (r#"[{"SetCreatureTypes":["Frog"]}]"#, vec!["Frog"], (2, 2)),
        (r#"[{"SetPT":{"power":1,"toughness":1}},{"SetColors":["Green"]},{"SetCreatureTypes":["Frog","Wizard"]}]"#, vec!["Frog", "Wizard"], (1, 1)),
        (r#"[{"SetCreatureTypes":[]}]"#, vec![], (2, 2))
    )]
    case: (&str, Vec<&str>, (i32, i32)),
) -> Result<(), ScriptError> {
    let (except, subtypes, size) = case;
    let scenario = || {
        Scenario::new(seats).battlefield(
            0,
            vec![
                ready(g12b_shifter(except)),
                ready(card("Grizzly Bears").unwrap()),
            ],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let bears = driver.id_of("Grizzly Bears")?;
    let target = Target::Object(driver.state().object(bears).unwrap().reference());
    let shifter = driver.id_of("Shifter")?;
    driver.begin_activation("Shifter")?;
    driver.choose_targets(&[target])?;
    run(&mut driver, seats)?;
    let copy = driver.state().object(shifter).unwrap().card();
    let expected: Vec<Subtype> = subtypes.iter().map(|s| Subtype((*s).into())).collect();
    assert_eq!(copy.subtypes, expected);
    assert_eq!((copy.power, copy.toughness), (Some(size.0), Some(size.1)));
    replays(&driver, &setup);
    Ok(())
}
