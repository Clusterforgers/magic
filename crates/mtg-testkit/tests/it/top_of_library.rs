//! Goal: a permission to cast or play from the top of the library (Sigarda, Font of Blessings; Mystic Forge; Future Sight; Bolas's Citadel; Vizier of the Menagerie) offers the top card in the priority options when it matches the permission and only then, never a card under it, a land only when the permission plays lands, and the cast takes the card to the stack from the library.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{lands, pay, resolve};
use crate::common::names;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::decision::{DecisionKind, PriorityOptions};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Sigarda's permission: Angel and Human spells.
const SIGARDA: &str = r#"{"what":[{"AnyOf":[{"Subtype":"Angel"},{"Subtype":"Human"}]},"TopOfLibrary",{"Not":{"Type":"Land"}}],"from":"Library"}"#;
/// Mystic Forge's permission: artifact and colorless spells.
const FORGE: &str = r#"{"what":[{"AnyOf":[{"Type":"Artifact"},"Colorless"]},"TopOfLibrary",{"Not":{"Type":"Land"}}],"from":"Library"}"#;
/// Future Sight's permission: play lands and cast spells.
const FUTURE_SIGHT: &str = r#"{"what":["TopOfLibrary"],"from":"Library","play":true}"#;
/// Bolas's Citadel's permission: play lands and cast spells for life.
const CITADEL: &str = r#"{"what":["TopOfLibrary"],"from":"Library","play":true,"rather":[{"PayLife":{"Stat":{"of":"This","stat":"ManaValue"}}}]}"#;
/// Vizier of the Menagerie's permission: creature spells.
const VIZIER: &str = r#"{"what":[{"Type":"Creature"},"TopOfLibrary"],"from":"Library"}"#;

/// The permanent that holds the permission.
fn holder(permission: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Holder")
            .with_type(CardType::Enchantment)
            .with_rules(
                serde_json::from_str(&format!(
                    r#"{{"Rule":{{"rule":{{"MayCast":{permission}}}}}}}"#
                ))
                .unwrap(),
            ),
    )
}

/// A white creature costing {W} with the subtype, or none.
fn white(name: &str, subtype: Option<&str>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_mana_cost(cost(0, &[Color::White]))
        .with_power_toughness(2, 2);
    if let Some(subtype) = subtype {
        card = card.with_subtype(subtype);
    }
    Arc::new(card)
}

/// A colorless artifact costing {1}.
fn trinket() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Trinket")
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(1, &[])),
    )
}

/// The library's top card in a case.
#[derive(Clone, Copy, Debug)]
enum Top {
    Angel,
    Human,
    Bear,
    Trinket,
    Land,
}

fn card(top: Top) -> Arc<CardDefinition> {
    match top {
        Top::Angel => white("Angel", Some("Angel")),
        Top::Human => white("Human", Some("Human")),
        Top::Bear => white("Bear", Some("Bear")),
        Top::Trinket => trinket(),
        Top::Land => plains(),
    }
}

fn table(permission: &str, top: Top) -> Result<Driver, ScriptError> {
    let mut mine = lands(2, plains);
    mine.push(ready(holder(permission)));
    Driver::from_scenario(
        Scenario::new(2)
            .library(0, [card(top), white("Under", Some("Angel"))])
            .battlefield(0, mine),
    )
}

fn options(driver: &Driver) -> PriorityOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => (**options).clone(),
        other => panic!("priority expected, got {}", other.name()),
    }
}

#[rstest]
#[case::sigarda_an_angel(SIGARDA, Top::Angel, true, false)]
#[case::sigarda_a_human(SIGARDA, Top::Human, true, false)]
#[case::sigarda_not_a_bear(SIGARDA, Top::Bear, false, false)]
#[case::sigarda_not_a_land(SIGARDA, Top::Land, false, false)]
#[case::forge_an_artifact(FORGE, Top::Trinket, true, false)]
#[case::forge_not_a_white_creature(FORGE, Top::Bear, false, false)]
#[case::future_sight_a_creature(FUTURE_SIGHT, Top::Bear, true, false)]
#[case::future_sight_a_land(FUTURE_SIGHT, Top::Land, false, true)]
#[case::citadel_a_creature(CITADEL, Top::Bear, true, false)]
#[case::citadel_a_land(CITADEL, Top::Land, false, true)]
#[case::vizier_a_creature(VIZIER, Top::Bear, true, false)]
#[case::vizier_not_an_artifact(VIZIER, Top::Trinket, false, false)]
fn the_top_card_is_offered_when_the_permission_covers_it(
    #[case] permission: &str,
    #[case] top: Top,
    #[case] castable: bool,
    #[case] playable: bool,
) -> Result<(), ScriptError> {
    let driver = table(permission, top)?;
    // The zone lists the bottom card first: `[Under, top]`.
    let library = driver.state().zone(Zone::Library(pid(0))).to_vec();
    let options = options(&driver);
    assert_eq!(
        options.castable.contains(&library[1]),
        castable,
        "{top:?} cast"
    );
    assert_eq!(
        options.playable_lands.contains(&library[1]),
        playable,
        "{top:?} play"
    );
    assert!(
        !options.castable.contains(&library[0]),
        "the card under the top is never offered"
    );
    Ok(())
}

#[rstest]
#[case::sigarda(SIGARDA, Top::Angel)]
#[case::future_sight(FUTURE_SIGHT, Top::Bear)]
#[case::vizier(VIZIER, Top::Human)]
fn the_offered_top_card_is_cast_from_the_library(
    #[case] permission: &str,
    #[case] top: Top,
) -> Result<(), ScriptError> {
    let mut driver = table(permission, top)?;
    let name = card(top).name.clone();
    driver.begin_cast(&name)?;
    pay(&mut driver)?;
    resolve(&mut driver)?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&name));
    assert_eq!(names(driver.state(), Zone::Library(pid(0))), ["Under"]);
    Ok(())
}
