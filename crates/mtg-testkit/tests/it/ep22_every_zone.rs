//! Goal: a static ability over every zone ("Creatures you control are artifacts. The same is true for creature spells you control and creature cards you own that aren't on the battlefield.") changes the creatures of its controller on the battlefield, in the hand, the library and the graveyard, and leaves another player's cards and noncreature cards alone; one zone named alone changes only that zone; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_testkit::cards::bear;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const EVERY: &str = r#"[{"In":"Battlefield"},{"In":"Stack"},{"In":"Hand"},{"In":"Library"},{"In":"Graveyard"},{"In":"Exile"},{"In":"Command"}]"#;

/// "Creatures you control are artifacts in addition to their other types" over the zones `zones`.
fn giver(zones: &str) -> Arc<CardDefinition> {
    let rule = format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Type":"Creature"}},{{"ControlledBy":"You"}},{{"AnyOf":{zones}}}]}},"mods":[{{"AddType":"Artifact"}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Giver")
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str::<Ability>(&rule).unwrap()),
    )
}

fn named(name: &str, kind: CardType) -> Arc<CardDefinition> {
    let card = CardDefinition::new(name).with_type(kind);
    Arc::new(if kind == CardType::Creature {
        card.with_power_toughness(2, 2)
    } else {
        card
    })
}

fn is_artifact(driver: &Driver, name: &str) -> bool {
    let id = driver
        .state()
        .objects()
        .find(|object| object.card().name == name)
        .unwrap()
        .id();
    mtg_engine::characteristics(driver.state(), id)
        .is_some_and(|chars| chars.types.contains(CardType::Artifact))
}

fn game(seats: usize, zones: &str) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [ready(giver(zones)), ready(bear())])
            .hand(
                0,
                [
                    named("Held", CardType::Creature),
                    named("Spell", CardType::Sorcery),
                ],
            )
            .library(0, [named("Deep", CardType::Creature)])
            .graveyard(0, [named("Dead", CardType::Creature)])
            .hand(1, [named("Theirs", CardType::Creature)]),
    )
}

#[rstest]
#[case::on_the_battlefield("Grizzly Bears", true)]
#[case::in_the_hand("Held", true)]
#[case::in_the_library("Deep", true)]
#[case::in_the_graveyard("Dead", true)]
#[case::not_a_creature("Spell", false)]
#[case::another_player_s("Theirs", false)]
fn every_zone_changes_the_controller_s_creatures(
    #[case] name: &str,
    #[case] artifact: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = game(seats, EVERY)?;
    assert_eq!(is_artifact(&driver, name), artifact);
    Ok(())
}

#[rstest]
#[case::the_hand_is_named("Held", true)]
#[case::the_graveyard_is_not("Dead", false)]
#[case::the_battlefield_is_not("Grizzly Bears", false)]
fn one_zone_named_changes_that_zone(
    #[case] name: &str,
    #[case] artifact: bool,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = game(seats, r#"[{"In":"Hand"}]"#)?;
    assert_eq!(is_artifact(&driver, name), artifact);
    Ok(())
}
