//! Goal: Doubling Cube doubles the amount of each type of unspent mana its controller has, each type counted with the
//! mana that may be spent only on some things, and the added mana carries no restriction (the Doubling Cube rulings);
//! it is a mana ability that resolves at once, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::ManaType;
use mtg_engine::decision::DecisionKind;
use mtg_testkit::cards::{forest, island, wastes};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A land whose colorless mana may be spent only to cast artifact spells.
fn workshop() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]},"only":[{"Spell":{"types":["Artifact"]}}]}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Workshop")
            .with_type(CardType::Land)
            .with_rules(rules),
    )
}

fn source(name: &str) -> Arc<CardDefinition> {
    match name {
        "Forest" => forest(),
        "Island" => island(),
        "Workshop" => workshop(),
        other => card(other).unwrap(),
    }
}

/// The unrestricted amount of each type and the number of restricted mana in seat 0's pool.
fn pool(driver: &Driver) -> ([u32; 6], usize) {
    let mana = driver.state().player(pid(0)).unwrap().mana();
    let types = [
        ManaType::White,
        ManaType::Blue,
        ManaType::Black,
        ManaType::Red,
        ManaType::Green,
        ManaType::Colorless,
    ];
    (types.map(|kind| mana.get(kind)), mana.restricted().len())
}

#[rstest]
#[case::green_doubles(&["Forest", "Forest"], [0, 0, 0, 0, 4, 0], 0)]
#[case::each_type_doubles(&["Forest", "Island"], [0, 2, 0, 0, 2, 0], 0)]
#[case::an_empty_pool_stays_empty(&[], [0; 6], 0)]
#[case::restricted_mana_counts_and_the_new_mana_is_free(&["Forest", "Workshop"], [0, 0, 0, 0, 2, 1], 1)]
fn r6603_the_cube_doubles_each_type(
    #[values(2, 3, 4)] seats: usize,
    #[case] tapped_first: &[&str],
    #[case] expected: [u32; 6],
    #[case] restricted: usize,
) -> Result<(), ScriptError> {
    let mut permanents: Vec<_> = tapped_first
        .iter()
        .map(|name| ready(source(name)))
        .collect();
    permanents.extend(["Doubling Cube"].iter().map(|name| ready(source(name))));
    permanents.extend((0..3).map(|_| ready(wastes())));
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(0, permanents))?;
    for name in tapped_first {
        driver.tap_for_mana(name)?;
    }
    driver.begin_activation("Doubling Cube")?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.pay_tapping(&["Wastes", "Wastes", "Wastes"], 0)?;
    }
    assert!(
        driver.state().stack().is_empty(),
        "a mana ability resolves at once"
    );
    assert_eq!(pool(&driver), (expected, restricted));
    Ok(())
}
