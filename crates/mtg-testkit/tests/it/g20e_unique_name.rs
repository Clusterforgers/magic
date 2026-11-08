//! Goal: "if it doesn't have the same name as another creature you control or a creature card in your graveyard"
//! leaves the entering creature out of its own comparison, so a first bear draws and a second one does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// Guardian Project's trigger, as the translator writes it.
fn project() -> Arc<CardDefinition> {
    let share = |pool: &str| format!(r#"{{"Not":{{"SharesName":{pool}}}}}"#);
    let graveyard = r#"{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"}]}"#;
    let other = r#"{"Each":[{"In":"Battlefield"},{"ControlledBy":"You"},{"Not":{"Is":"Candidate"}}]}"#;
    let rules = format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"to":"Battlefield","what":[{{"Type":"Creature"}},{{"Not":"Token"}},{{"ControlledBy":"You"}},{},{}]}}}},"effects":[{{"Draw":{{}}}}]}}}}"#,
        share(graveyard),
        share(other)
    );
    Arc::new(
        CardDefinition::new("Project")
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str(&rules).unwrap()),
    )
}

#[rstest]
#[case::first_of_its_name(false, false, 1)]
#[case::another_on_the_battlefield(true, false, 0)]
#[case::one_in_the_graveyard(false, true, 0)]
fn a_new_name_draws(
    #[case] on_battlefield: bool,
    #[case] in_graveyard: bool,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let mut field = vec![ready(project()), ready(forest()), ready(forest())];
    if on_battlefield {
        field.push(ready(bear()));
    }
    let scenario = Scenario::new(2)
        .battlefield(0, field)
        .hand(0, [bear()])
        .library(0, [forest(), forest()])
        .graveyard(0, if in_graveyard { vec![bear()] } else { Vec::new() });
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Grizzly Bears")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))).len(), drawn);
    Ok(())
}
