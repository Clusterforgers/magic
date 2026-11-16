//! Goal: Doubling Cube plays as printed at two to four seats: its mana ability adds, for each type of mana, as much
//! more as its controller has unspent, restricted mana counted but not copied (the Doubling Cube rulings), the mana that
//! paid its cost no longer counts, and no other player's pool changes.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::ManaType;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{island, swamp, wastes};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A land that taps for {U} that may be spent only to cast creature spells.
fn r6501_creature_isle() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Blue"]},"only":[{"Spell":{"types":["Creature"]}}]}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Creature Isle")
            .with_type(CardType::Land)
            .with_rules(rules),
    )
}

/// The free mana of `kind` and the restricted mana in the seat's pool.
fn r6501_pool(driver: &Driver, seat: usize, kind: ManaType) -> (u32, usize) {
    let pool = driver.state().player(pid(seat)).unwrap().mana();
    let restricted = pool
        .restricted()
        .iter()
        .filter(|unit| unit.mana == kind)
        .count();
    (pool.get(kind), restricted)
}

#[rstest]
fn r6501_doubling_cube_doubles_each_type_of_unspent_mana(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Doubling Cube").unwrap()),
            ready(swamp()),
            ready(swamp()),
            ready(island()),
            ready(r6501_creature_isle()),
            ready(wastes()),
            ready(wastes()),
            ready(wastes()),
        ],
    );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.tap_for_mana("Swamp")?;
    driver.tap_for_mana("Swamp")?;
    driver.tap_for_mana("Island")?;
    driver.tap_for_mana("Creature Isle")?;
    assert_eq!(r6501_pool(&driver, 0, ManaType::Black), (2, 0));
    assert_eq!(r6501_pool(&driver, 0, ManaType::Blue), (1, 1));
    driver.begin_activation("Doubling Cube")?;
    driver.pay_tapping(&["Wastes", "Wastes", "Wastes"], 0)?;
    // A mana ability: it resolves at once, never on the stack (CR 605.3).
    assert!(driver.state().stack().is_empty());
    // {B}{B}{U} and a creature-only {U} become {B}{B}{B}{B}{U}{U}{U} and the creature-only {U}; the {C} that paid the
    // cost is spent, so no colorless mana is added.
    assert_eq!(r6501_pool(&driver, 0, ManaType::Black), (4, 0));
    assert_eq!(r6501_pool(&driver, 0, ManaType::Blue), (3, 1));
    assert_eq!(r6501_pool(&driver, 0, ManaType::Colorless), (0, 0));
    for seat in 1..seats {
        assert_eq!(r6501_pool(&driver, seat, ManaType::Black), (0, 0));
    }
    Ok(())
}

#[rstest]
fn r6501_doubling_cube_with_an_empty_pool_adds_nothing(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Doubling Cube").unwrap()),
            ready(wastes()),
            ready(wastes()),
            ready(wastes()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Doubling Cube")?;
    driver.pay_tapping(&["Wastes", "Wastes", "Wastes"], 0)?;
    assert!(driver.state().player(pid(0)).unwrap().mana().is_empty());
    Ok(())
}
