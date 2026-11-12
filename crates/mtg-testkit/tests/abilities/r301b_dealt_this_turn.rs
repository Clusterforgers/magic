//! Goal: "if ~ has dealt N or more damage this turn" counts the damage the source itself has dealt this turn, the damage
//! of the resolving ability included, and not the damage of another source, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A creature that, for {T}, deals `damage` to target player, then gains 5 life if it has dealt 3 or more damage this
/// turn.
fn pinger(name: &str, damage: i32) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_value(serde_json::json!({"Activated": {
        "cost": ["Tap"],
        "targets": [{"what": "Player"}],
        "effects": [
            {"DealDamage": {"to": {"Target": 0}, "amount": damage}},
            {"If": {
                "condition": {"Compare": {
                    "left": {"Tally": {"event": "Damaged", "who": "EachPlayer", "filter": ["This"]}},
                    "op": "Ge",
                    "right": 3}},
                "then": [{"GainLife": {"amount": 5}}]}}]}}))
    .unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability),
    )
}

fn life(driver: &Driver, seat: u64) -> i32 {
    driver.state().player(PlayerId(seat)).unwrap().life()
}

fn activate(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    let target = driver.target_player(1);
    driver.choose_targets(&[target])?;
    for _ in 0..20 {
        if !driver.state().stack().is_empty() {
            driver.answer_passively()?;
        }
    }
    Ok(())
}

#[rstest]
#[case::two_is_too_little(&[2], 0)]
#[case::three_at_once(&[3], 5)]
#[case::another_source_does_not_count(&[2, 2], 0)]
fn the_source_counts_its_own_damage_this_turn(
    #[values(2, 3, 4)] seats: usize,
    #[case] damage: &[i32],
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    let pingers: Vec<_> = damage
        .iter()
        .enumerate()
        .map(|(at, amount)| ready(pinger(&format!("Pinger {at}"), *amount)))
        .collect();
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, pingers)
        .battlefield(1, [ready(free_creature("Bear", 2, 2))]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..20).map(|_| island()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let before = life(&driver, 0);
    for at in 0..damage.len() {
        activate(&mut driver, &format!("Pinger {at}"))?;
    }
    assert_eq!(life(&driver, 0) - before, gained);
    Ok(())
}
