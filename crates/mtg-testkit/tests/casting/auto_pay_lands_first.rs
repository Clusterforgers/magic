//! Goal: "pay automatically" taps lands before the artifacts and creatures that make mana (a Sol Ring, a Signet, a mana creature, a Treasure) whenever the cost can still be paid, whichever of them came into play first, at tables of two to four seats; a colored symbol still takes the source that makes it, and a cost no land set could pay is paid with the rest.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest, island, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::names;

/// An artifact with "{T}, Sacrifice this artifact: Add one mana of any color."
fn treasure() -> Arc<CardDefinition> {
    let rule = r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#;
    Arc::new(
        CardDefinition::new("Treasure")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(rule).unwrap()),
    )
}

/// The permanent of the table with this name.
fn permanent(name: &str) -> mtg_engine::PresetPermanent {
    ready(match name {
        "Forest" => forest(),
        "Mountain" => mountain(),
        "Island" => island(),
        "Plains" => plains(),
        "Treasure" => treasure(),
        other => card(other).unwrap(),
    })
}

/// Whether the named permanent was used to pay: it is tapped or it left the battlefield (a Treasure is sacrificed).
fn used(driver: &Driver, name: &str) -> bool {
    if !names(driver.state(), Zone::Battlefield).contains(&name.to_owned()) {
        return true;
    }
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

// The cost is `generic` and one {G} for each `green`; the first permanents of a table are the first in play.
#[rstest]
#[case::sol_ring_stays(2, 0, &["Sol Ring", "Forest", "Mountain"], &["Forest", "Mountain"])]
#[case::sol_ring_pays_the_rest(3, 0, &["Sol Ring", "Forest"], &["Sol Ring", "Forest"])]
#[case::a_creature_stays(1, 1, &["Llanowar Elves", "Forest", "Mountain"], &["Forest", "Mountain"])]
#[case::a_creature_pays_the_color_a_land_cannot(0, 2, &["Llanowar Elves", "Forest", "Mountain"], &["Llanowar Elves", "Forest"])]
#[case::a_signet_stays(3, 0, &["Azorius Signet", "Island", "Plains", "Mountain"], &["Island", "Plains", "Mountain"])]
#[case::a_signet_makes_what_lands_cannot(4, 0, &["Azorius Signet", "Island", "Plains", "Mountain"], &["Azorius Signet", "Island", "Plains", "Mountain"])]
#[case::a_treasure_stays(1, 0, &["Treasure", "Forest"], &["Forest"])]
#[case::a_treasure_pays_the_last(2, 0, &["Treasure", "Forest"], &["Treasure", "Forest"])]
#[case::sol_ring_before_a_treasure(3, 0, &["Treasure", "Sol Ring", "Forest"], &["Sol Ring", "Forest"])]
#[case::the_rest_stay_for_the_lands(2, 0, &["Llanowar Elves", "Treasure", "Sol Ring", "Forest", "Island"], &["Forest", "Island"])]
fn pay_automatically_taps_lands_before_artifacts_and_creatures(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] lands_first: bool,
    #[case] generic: u32,
    #[case] green: usize,
    #[case] table: &[&str],
    #[case] paid_with: &[&str],
) -> Result<(), ScriptError> {
    let is_land = |name: &&str| matches!(*name, "Forest" | "Mountain" | "Island" | "Plains");
    let mut order = table.to_vec();
    if lands_first {
        order.sort_by_key(|name| !is_land(name));
    }
    let spell = creature("Spell", generic, &vec![Color::Green; green], 1, 1);
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [spell])
            .battlefield(0, order.iter().map(|name| permanent(name))),
    )?;
    driver.cast("Spell")?;
    for name in table {
        assert_eq!(
            used(&driver, name),
            paid_with.contains(name),
            "{name} in {table:?}"
        );
    }
    Ok(())
}
