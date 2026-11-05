//! Goal: damage shields that multiply or redirect damage are played (and a shape the engine lacks stays by hand), a rule the engine cannot play never stops a game, and the fixes of the round 7 audit run in real games with the case where they must not apply.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::card::Card;
use mtg_cards::support::{PrintedCard, PrintedFace, oracle_fingerprint};
use mtg_cards::{CardDefinition, builtin};
use mtg_core::ids::PlayerId;
use mtg_testkit::cards::mountain;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::{Value, json};

use crate::r3p3_runs::settle;

/// The definition the engine runs for these abilities, or why it would not run them.
pub(crate) fn defined(
    abilities: &Value,
    name: &str,
    types: &str,
    cost: &str,
) -> Result<Arc<CardDefinition>, String> {
    let oracle_id = format!("00000000-0000-4000-8000-{:012}", name.len());
    let card: Card = serde_json::from_value(json!({
        "oracle_id": oracle_id, "name": name, "impl_status": "Full",
        "faces": [{"name": name, "oracle_fingerprint": oracle_fingerprint(name), "abilities": abilities}]}))
    .unwrap();
    let printed = PrintedCard {
        oracle_id,
        name: name.to_owned(),
        keywords: vec![],
        layout: "normal".to_owned(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            mana_cost: cost.to_owned(),
            type_line: types.to_owned(),
            oracle_text: name.to_owned(),
            power: types.contains("Creature").then(|| "1".to_owned()),
            toughness: types.contains("Creature").then(|| "1".to_owned()),
            loyalty: None,
            defense: None,
            colors: vec![],
        }],
    };
    mtg_cli::drafts::read_definitions(
        builtin::library().unwrap(),
        &mtg_engine::caps(),
        &[(&printed, &card)],
    )
    .into_iter()
    .next()
    .unwrap()
    .map(Arc::new)
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(seat))
        .unwrap()
        .life()
}

#[rstest]
#[case::double(json!({"Multiply": 2}), Some(14))]
#[case::redirected(
    json!({"Instead": [{"DealDamage": {"amount": "Triggering", "source": "Dealer", "to": "You"}}]}),
    Some(20)
)]
#[case::half_prevented_waits(json!("PreventHalf"), None)]
fn a_damage_shield_is_played_or_left_by_hand(#[case] by: Value, #[case] opponent: Option<i32>) {
    let rules = json!([{"Spell": {"effects": [
        {"Shield": {"when": {"Damage": {"source": [{"ControlledBy": "You"}], "to": "Opponents"}},
            "by": by, "duration": "UntilEndOfTurn"}},
        {"DealDamage": {"to": "Opponents", "amount": 3}}]}}]);
    let spell = defined(&rules, "Bolt Shield", "Instant", "{R}");
    let Some(opponent) = opponent else {
        assert!(spell.is_err(), "the engine lacks this shield");
        return;
    };
    let scenario = Scenario::new(2)
        .hand(0, [spell.unwrap()])
        .battlefield(0, [ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Bolt Shield").unwrap();
    settle(&mut driver);
    assert_eq!(life(&driver, 1), opponent);
}

#[rstest]
#[case::fixed(json!(1), true)]
#[case::half_of_a_stat(json!({"Half": {"up": true, "of": {"Stat": {"of": {"Bound": "d"}, "stat": "Power"}}}}), false)]
fn a_copy_with_a_computed_size_stays_by_hand(#[case] power: Value, #[case] runs: bool) {
    let rules = json!([{"Spell": {"effects": [
        {"Destroy": {"what": "This", "as": "d"}},
        {"CreateToken": {"token": {"CopyOf": {"of": {"Bound": "d"}, "except": [
            {"SetPT": {"power": power, "toughness": 1}}]}}}}]}}]);
    assert_eq!(defined(&rules, "Saw Half", "Instant", "{R}").is_ok(), runs);
}

use crate::r3p3_runs::translated;

#[rstest]
#[case::the_third_counter_one_at_a_time(1, 3, 5)]
#[case::two_is_not_the_third(1, 2, 0)]
#[case::three_at_once_is_a_total_of_three(3, 1, 5)]
#[case::steps_over_three(2, 2, 0)]
fn the_nth_counter_is_the_total_on_the_object(
    #[case] per_step: u32,
    #[case] steps: usize,
    #[case] gained: i32,
) {
    let script = format!(
        "Name:Plan Board\nTypes:Enchantment\nA:AB$ PutCounter | Cost$ 0 | CounterType$ PLAN | CounterNum$ {per_step}\nT:Mode$ CounterAdded | ValidCard$ Card.Self | TriggerZones$ Battlefield | CounterType$ PLAN | CounterAmount$ EQ3 | Execute$ TrigGain | TriggerDescription$ When the third plan counter is put on this enchantment, you gain 5 life.\nSVar:TrigGain:DB$ GainLife | LifeAmount$ 5\n"
    );
    let board = translated(&script, "Plan Board", "Enchantment", "{0}");
    let scenario = Scenario::new(2).battlefield(0, [ready(board)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    for _ in 0..steps {
        driver.begin_activation("Plan Board").unwrap();
        driver.pay_tapping(&[], 0).unwrap();
        settle(&mut driver);
    }
    assert_eq!(life(&driver, 0), 20 + gained);
}
