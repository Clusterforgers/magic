//! Goal: the two-trigger exile of Journey to Nowhere, as the compiler reads it from the Oracle text, plays whole at tables of two, three and four seats: the creature waits in exile while the enchantment stays, returns as a new object when it leaves, never leaves the battlefield when the enchantment is gone before its trigger resolves, and the game replays from its events.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::compile::compile;
use mtg_cards::support::{PrintedCard, PrintedFace};
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

const JOURNEY: &str = "When this enchantment enters, exile target creature.\nWhen this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control.";

/// A card for `{0}` with the abilities the compiler reads from this text.
fn compiled(name: &str, card_type: CardType, type_line: &str, text: &str) -> Arc<CardDefinition> {
    let printed = PrintedCard {
        oracle_id: "12345678-1234-1234-1234-123456789abe".to_owned(),
        name: name.to_owned(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            type_line: type_line.to_owned(),
            oracle_text: text.to_owned(),
            ..PrintedFace::default()
        }],
        ..PrintedCard::default()
    };
    let card = compile(&printed).card.expect("the card compiles");
    let mut definition = CardDefinition::new(name)
        .with_type(card_type)
        .with_mana_cost(cost(0, &[]));
    for ability in &card.faces[0].abilities {
        definition = definition.with_rules(ability.clone());
    }
    Arc::new(definition)
}

/// Seat 0 holds Journey to Nowhere and six Plains; seat 1 holds a bounce spell and controls a Bear. The game stands in
/// seat 0's first main phase.
fn table(seats: usize) -> Result<Driver, ScriptError> {
    let journey = compiled(
        "Journey to Nowhere",
        CardType::Enchantment,
        "Enchantment",
        JOURNEY,
    );
    let bounce = compiled(
        "Unsummon Permanent",
        CardType::Instant,
        "Instant",
        "Return target permanent to its owner's hand.",
    );
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [journey])
        .battlefield(0, (0..6).map(|_| ready(plains())))
        .hand(1, [bounce])
        .battlefield(1, [ready(free_creature("Bear", 2, 2))]);
    Driver::from_scenario(scenario)
}

/// Every seat passes once, so the top of the stack resolves.
fn resolve(driver: &mut Driver, seats: usize) {
    for _ in 0..seats {
        driver.pass().unwrap();
    }
}

/// Seat 0 casts the enchantment and it resolves; its trigger is put on the stack aimed at the Bear.
fn cast_at_the_bear(driver: &mut Driver, seats: usize) {
    driver.cast("Journey to Nowhere").unwrap();
    resolve(driver, seats);
    let bear = driver.target_named("Bear").unwrap();
    driver.choose_targets(&[bear]).unwrap();
}

/// Seat 1 gets priority with the stack as it is and sends the enchantment back to its owner's hand.
fn bounce_the_enchantment(driver: &mut Driver, seats: usize) {
    driver.pass().unwrap();
    let aimed = driver.target_named("Journey to Nowhere").unwrap();
    driver.cast_at("Unsummon Permanent", &[aimed]).unwrap();
    resolve(driver, seats);
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn in_play(driver: &Driver, name: &str) -> bool {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| name_of(driver.state(), *id) == name)
}

#[rstest]
fn the_creature_waits_in_exile_and_returns_as_a_new_object(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats)?;
    let bear = driver.id_of("Bear")?;
    let before = driver.state().object(bear).unwrap().counter();
    cast_at_the_bear(&mut driver, seats);
    resolve(&mut driver, seats);
    let held = driver.state().object(bear).unwrap();
    assert_eq!(held.zone(), Zone::Exile);
    assert!(held.exiled_until().is_some(), "linked to the enchantment");
    assert!(in_play(&driver, "Journey to Nowhere"));
    bounce_the_enchantment(&mut driver, seats);
    assert!(!in_play(&driver, "Journey to Nowhere"));
    let back = driver.state().object(bear).unwrap();
    assert_eq!(back.zone(), Zone::Battlefield, "the Bear came back");
    assert!(back.counter() > before + 1, "as a new object");
    assert_eq!(back.owner(), pid(1));
    assert_eq!(back.controller(), pid(1));
    assert_eq!(driver.state().zone(Zone::Exile).len(), 0);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn an_enchantment_gone_before_its_trigger_resolves_exiles_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats)?;
    let bear = driver.id_of("Bear")?;
    let counter = driver.state().object(bear).unwrap().counter();
    cast_at_the_bear(&mut driver, seats);
    bounce_the_enchantment(&mut driver, seats);
    assert!(!in_play(&driver, "Journey to Nowhere"));
    resolve(&mut driver, seats);
    let bear = driver.state().object(bear).unwrap();
    assert_eq!(bear.zone(), Zone::Battlefield);
    assert_eq!(bear.counter(), counter, "the Bear never left");
    assert_eq!(driver.state().zone(Zone::Exile).len(), 0);
    assert_replays(&driver);
    Ok(())
}
