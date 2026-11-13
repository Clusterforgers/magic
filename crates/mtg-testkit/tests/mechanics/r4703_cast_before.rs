//! Goal: "copy it for each other instant and sorcery spell you've cast before it this turn" (Thousand-Year Storm) counts
//! only the caster's instants and sorceries cast before the triggering spell, not one cast in response to the trigger,
//! not creature spells and not an opponent's spells, at 2 to 4 seats; and a card that counts as a card named Flame Burst
//! in a graveyard is counted only by a spell of that name (Pardic Firecat).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A free spell of `kind` named `name` with the effects `effects` (JSON).
fn spell(name: &str, kind: CardType, effects: &str) -> Arc<CardDefinition> {
    let rules: Ability =
        serde_json::from_str(&format!(r#"{{"Spell":{{"effects":{effects}}}}}"#)).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

/// A free instant: 1 damage to each opponent.
fn spark(name: &str) -> Arc<CardDefinition> {
    spell(
        name,
        CardType::Instant,
        r#"[{"DealDamage":{"to":"Opponents","amount":1}}]"#,
    )
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// Every spell, trigger and copy resolves.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().zone(Zone::Stack).is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Ok(())
}

/// Seat 0 controls Thousand-Year Storm and casts `sparks` sparks one after another (each resolving with its copies).
#[rstest]
#[case::one(1, 1)]
#[case::two(2, 3)]
#[case::three(3, 6)]
fn each_spell_is_copied_once_for_each_before_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] sparks: usize,
    #[case] damage: i32,
) -> Result<(), ScriptError> {
    let names: Vec<String> = (0..sparks).map(|n| format!("Spark{n}")).collect();
    let hand: Vec<_> = names.iter().map(|name| spark(name)).collect();
    let scenario = Scenario::new(seats)
        .battlefield(0, vec![ready(card("Thousand-Year Storm").unwrap())])
        .hand(0, hand);
    let mut driver = Driver::from_scenario(scenario)?;
    for name in &names {
        driver.cast(name)?;
        settle(&mut driver)?;
    }
    for seat in 1..seats {
        assert_eq!(life(&driver, seat), 20 - damage, "seat {seat}");
    }
    Ok(())
}

/// A creature spell between the sparks is no instant or sorcery: the second spark is copied once.
#[rstest]
fn a_creature_spell_is_not_counted(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(0, vec![ready(card("Thousand-Year Storm").unwrap())])
        .hand(
            0,
            vec![
                spark("Spark0"),
                free_creature("Bear", 2, 2),
                spark("Spark1"),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    for name in ["Spark0", "Bear", "Spark1"] {
        driver.cast(name)?;
        settle(&mut driver)?;
    }
    assert_eq!(life(&driver, 1), 20 - 3);
    Ok(())
}

/// Spark1 is cast in response to Spark0's trigger: Spark1 is copied once, Spark0 not at all, though Spark1 was cast this
/// turn by the time Spark0's trigger resolves.
#[rstest]
fn a_spell_cast_later_is_not_counted(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(0, vec![ready(card("Thousand-Year Storm").unwrap())])
        .hand(0, vec![spark("Spark0"), spark("Spark1")]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Spark0")?;
    driver.cast("Spark1")?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 1), 20 - 3);
    Ok(())
}

/// "Flame Burst deals X damage ..., where X is 2 plus the number of cards named Flame Burst in all graveyards."
fn burst(name: &str) -> Arc<CardDefinition> {
    spell(
        name,
        CardType::Instant,
        r#"[{"DealDamage":{"to":"Opponents","amount":{"Sum":[2,{"Count":[{"In":"Graveyard"},{"Named":"Flame Burst"}]}]}}}]"#,
    )
}

/// Pardic Firecat in a graveyard counts for a spell named Flame Burst only, in any player's graveyard, and not on the
/// battlefield (Pardic Firecat's ruling: the card counts only while it is in a graveyard).
#[rstest]
#[case::flame_burst_counts_it("Flame Burst", 1, 0, 3)]
#[case::in_an_opponents_graveyard("Flame Burst", 1, 1, 3)]
#[case::two_of_them("Flame Burst", 2, 0, 4)]
#[case::another_name_does_not("Flame Bursts", 1, 0, 2)]
#[case::none("Flame Burst", 0, 0, 2)]
fn a_card_counts_as_named_for_spells_of_that_name(
    #[values(2, 3, 4)] seats: usize,
    #[case] name: &str,
    #[case] firecats: usize,
    #[case] whose: usize,
    #[case] damage: i32,
) -> Result<(), ScriptError> {
    let cats: Vec<_> = (0..firecats)
        .map(|_| card("Pardic Firecat").unwrap())
        .collect();
    let scenario = Scenario::new(seats)
        .graveyard(whose, cats)
        .battlefield(0, vec![ready(card("Pardic Firecat").unwrap())])
        .hand(0, vec![burst(name)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast(name)?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 1), 20 - damage);
    Ok(())
}
