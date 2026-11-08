//! Goal: a permanent that becomes a copy may keep what the exceptions of the copy say (CR 707.9b): another name, the legendary supertype and the ability that made it a copy, each alone and all together, and the layers still apply on top.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::{replays, run};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Supertype};
use mtg_engine::choose::Target;
use mtg_testkit::corpus::card;
use mtg_testkit::trigger_cards::sized_creature_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

/// A 1/1 with "{0}: This becomes a copy of another target creature, except `except`."
fn shifter(except: &str) -> Arc<CardDefinition> {
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
#[case::plain("[]", "Grizzly Bears", false, false)]
#[case::named(r#"[{"SetName":"Mime"}]"#, "Mime", false, false)]
#[case::legendary(r#"[{"AddSupertype":"Legendary"}]"#, "Grizzly Bears", true, false)]
#[case::keeps_the_ability(r#"["ThisAbility"]"#, "Grizzly Bears", false, true)]
#[case::all_three(
    r#"[{"SetName":"Mime"},{"AddSupertype":"Legendary"},"ThisAbility"]"#,
    "Mime",
    true,
    true
)]
fn a_copy_may_be_named_legendary_and_keep_the_ability_that_made_it(
    #[case] except: &str,
    #[case] name: &str,
    #[case] legendary: bool,
    #[case] keeps_ability: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2).battlefield(
            0,
            vec![
                ready(shifter(except)),
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
    run(&mut driver, 2)?;
    let copy = driver.state().object(shifter).unwrap().card();
    assert_eq!(copy.name, name);
    assert_eq!((copy.power, copy.toughness), (Some(2), Some(2)));
    assert!(copy.types.contains(&CardType::Creature));
    assert_eq!(copy.supertypes.contains(&Supertype::Legendary), legendary);
    assert_eq!(
        driver
            .offered_abilities(name)
            .is_ok_and(|offered| !offered.is_empty()),
        keeps_ability
    );
    replays(&driver, &setup);
    Ok(())
}
