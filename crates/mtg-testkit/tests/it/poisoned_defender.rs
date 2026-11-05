//! Goal: "when this attacks, if defending player is poisoned, it gets +1/+1" grows the attacker only when the player it attacks has a poison counter, whichever seat that is.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::characteristics;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The condition the translator writes for `DefendingPlayerPoisoned$ True`.
const RAT: &str = r#"{"Triggered": {"when": {"Attacks": {"what": ["This"]}}, "if": {"Compare": {"left": {"Counters": {"on": "Defending", "counter": "poison"}}, "op": "Ge", "right": 1}}, "effects": [{"Continuous": {"affected": "This", "duration": "UntilEndOfTurn", "mods": [{"ModifyPT": {"power": 1, "toughness": 1}}]}}]}}"#;

#[rstest]
#[case::poisoned_defender(1, 1, 3)]
#[case::other_player_poisoned(2, 1, 2)]
#[case::clean_defender(1, 0, 2)]
fn the_attacker_grows_only_when_its_defender_is_poisoned(
    #[case] poisoned_seat: usize,
    #[case] poison: u32,
    #[case] power: i32,
) -> Result<(), ScriptError> {
    let ability: Ability = serde_json::from_str(RAT).unwrap();
    let rat = Arc::new(
        CardDefinition::new("Septic")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[]))
            .with_power_toughness(2, 2)
            .with_rules(ability),
    );
    let mut scenario = Scenario::new(3).at(1, 0, Step::DeclareAttackers);
    scenario = scenario.battlefield(0, [ready(rat)]);
    for seat in 0..3 {
        scenario = scenario.library(seat, (0..12).map(|_| forest()));
    }
    if poison > 0 {
        scenario = scenario.counters(poisoned_seat, CounterKind::Poison, poison);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Septic", 1)])?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
            break;
        }
        driver.pass()?;
    }
    let id = driver.id_of("Septic")?;
    assert_eq!(
        characteristics(driver.state(), id).unwrap().power,
        Some(power)
    );
    Ok(())
}
