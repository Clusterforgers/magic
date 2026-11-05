//! Goal: the size an event must have (`DamageAmount$`, `CounterAmount$`, `LifeAmount$`) and `FirstTime$ True` read once for every trigger mode that has them: each is a condition of the event that holds for the sizes the card prints and not for the others, and a mode or value the node cannot say stays a gap named after the key.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_testkit::cards::{burn, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::{Value, json};

use crate::forge::script::Script;
use crate::translate::card::translate_script;
use crate::translate::ctx::World;

fn world() -> World {
    let mut world = World::new(&[], std::collections::BTreeMap::default());
    world.subtypes.insert("Desert".to_owned());
    world
}

fn abilities(text: &str) -> Vec<Value> {
    let out = translate_script(&world(), &Script::parse(text));
    out.faces
        .into_iter()
        .next()
        .map(|face| face.abilities)
        .unwrap_or_default()
}

fn gaps(text: &str) -> Vec<String> {
    translate_script(&world(), &Script::parse(text))
        .gaps()
        .map(|gap| gap.0.clone())
        .collect()
}

fn trigger(mode: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact Desert\nT:Mode$ {mode} | TriggerZones$ Battlefield | Execute$ G\nSVar:G:DB$ GainLife | LifeAmount$ 5\n"
    )
}

fn size(op: &str, number: u32) -> Value {
    json!({"AtTrigger": {"Compare": {"left": "Triggering", "op": op, "right": number}}})
}

#[rstest]
#[case::damage_at_least(
    "DamageDone | ValidSource$ Card | ValidTarget$ Player | DamageAmount$ GE5",
    "Ge",
    5
)]
#[case::damage_exactly(
    "DamageDone | ValidSource$ Card | ValidTarget$ Player | DamageAmount$ EQ1",
    "Eq",
    1
)]
#[case::damage_once(
    "DamageDoneOnce | ValidSource$ Card | ValidTarget$ Player | DamageAmount$ GE3",
    "Ge",
    3
)]
#[case::life_lost("LifeLost | ValidPlayer$ You | LifeAmount$ EQ1", "Eq", 1)]
#[case::life_gained("LifeGained | ValidPlayer$ You | LifeAmount$ LE2", "Le", 2)]
fn the_size_is_a_condition_of_the_event(#[case] mode: &str, #[case] op: &str, #[case] number: u32) {
    let text = trigger(mode);
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text)[0]["Triggered"]["if"], size(op, number));
}

#[rstest]
#[case::variable(
    "DamageDone | ValidSource$ Card | DamageAmount$ EQT",
    "key:Trigger.DamageAmount=EQT"
)]
#[case::unknown_operator(
    "LifeLost | ValidPlayer$ You | LifeAmount$ XX1",
    "key:Trigger.LifeAmount=XX1"
)]
#[case::new_total(
    "CounterRemoved | ValidCard$ Creature | NewCounterAmount$ 0",
    "key:Trigger.Counter.NewCounterAmount"
)]
#[case::first_time_with_source(
    "BecomesTarget | ValidTarget$ Card.Self | ValidSource$ Card.YouCtrl | FirstTime$ True",
    "key:Trigger.BecomesTarget.FirstTime-with-source"
)]
#[case::first_time_value(
    "LifeGained | ValidPlayer$ You | FirstTime$ Maybe",
    "key:Trigger.FirstTime=Maybe"
)]
fn what_the_node_cannot_say_is_a_gap(#[case] mode: &str, #[case] gap: &str) {
    assert!(
        gaps(&trigger(mode)).iter().any(|found| found == gap),
        "{:?}",
        gaps(&trigger(mode))
    );
}

#[rstest]
#[case::life("LifeGained | ValidPlayer$ You | FirstTime$ True", json!({"Nth": {"event": {"LifeGained": {"who": "You"}}, "nth": 1}}))]
#[case::target(
    "BecomesTarget | ValidTarget$ Card.Self | FirstTime$ True",
    json!({"Nth": {"event": {"BecomesTarget": {"what": ["This"]}}, "nth": 1}})
)]
fn first_time_is_the_first_of_the_turn(#[case] mode: &str, #[case] when: Value) {
    let text = trigger(mode);
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text)[0]["Triggered"]["when"], when);
}

fn subject(mode: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Subject").with_type(CardType::Artifact);
    for value in abilities(&trigger(mode)) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

/// Casts a bolt of `damage` at the opponent with the subject in play and returns the life of its controller.
fn life_after_bolt(mode: &str, damage: i32) -> i32 {
    let scenario = Scenario::new(2)
        .hand(0, [burn("Bolt", damage)])
        .battlefield(0, [ready(subject(mode)), ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_player(1);
    driver.begin_cast("Bolt").unwrap();
    driver.choose_targets(&[target]).unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    for _ in 0..40 {
        driver.answer_passively().unwrap();
    }
    driver
        .state()
        .player(PlayerId::from_index(0))
        .unwrap()
        .life()
}

#[rstest]
#[case::big_enough(3, 25)]
#[case::bigger(5, 25)]
#[case::too_small(2, 20)]
fn damage_below_the_printed_size_does_not_trigger(#[case] damage: i32, #[case] life: i32) {
    let mode = "DamageDone | ValidSource$ Card | ValidTarget$ Player | DamageAmount$ GE3";
    assert_eq!(life_after_bolt(mode, damage), life);
}

#[rstest]
#[case::exactly(2, 25)]
#[case::more(3, 20)]
fn exact_damage_triggers_only_for_that_size(#[case] damage: i32, #[case] life: i32) {
    let mode = "DamageDone | ValidSource$ Card | ValidTarget$ Player | DamageAmount$ EQ2";
    assert_eq!(life_after_bolt(mode, damage), life);
}

fn desert() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Parched Dunes")
            .with_type(CardType::Land)
            .with_subtype("Desert"),
    )
}

const IF_DESERT: &str = "DamageDone | ValidSource$ Card | ValidTarget$ Player | IsPresent$ Desert | PresentZone$ Battlefield,Graveyard | PresentPlayer$ You";

#[test]
fn the_zones_of_a_present_check_are_alternatives() {
    let text = trigger(IF_DESERT);
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = &abilities(&text)[0]["Triggered"]["if"];
    assert_eq!(found["Any"].as_array().map(Vec::len), Some(2), "{found}");
    assert!(found.to_string().contains("OwnedBy"), "{found}");
    assert!(found.to_string().contains("ControlledBy"), "{found}");
}

#[rstest]
#[case::opponent(
    "IsPresent$ Desert | PresentPlayer$ Opponent",
    "key:Condition.PresentPlayer=Opponent"
)]
#[case::count_over_two_zones(
    "IsPresent$ Desert | PresentZone$ Battlefield,Graveyard | PresentCompare$ GE2",
    "zone:Battlefield,Graveyard"
)]
fn a_present_check_it_cannot_say_is_a_gap(#[case] keys: &str, #[case] gap: &str) {
    let text = trigger(&format!("DamageDone | ValidSource$ Card | {keys}"));
    assert!(
        gaps(&text).iter().any(|found| found == gap),
        "{:?}",
        gaps(&text)
    );
}

/// The life of seat 0 after a bolt of 1 at the opponent, with a Desert where the case puts it.
fn life_with_desert(mine: bool, theirs: bool, battlefield: bool) -> i32 {
    let mut field = vec![ready(subject(IF_DESERT)), ready(mountain())];
    if mine && battlefield {
        field.push(ready(desert()));
    }
    let mut scenario = Scenario::new(2)
        .hand(0, [burn("Bolt", 1)])
        .battlefield(0, field);
    if mine && !battlefield {
        scenario = scenario.graveyard(0, [desert()]);
    }
    if theirs {
        scenario = scenario.graveyard(1, [desert()]);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_player(1);
    driver.begin_cast("Bolt").unwrap();
    driver.choose_targets(&[target]).unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    for _ in 0..40 {
        driver.answer_passively().unwrap();
    }
    driver
        .state()
        .player(PlayerId::from_index(0))
        .unwrap()
        .life()
}

#[rstest]
#[case::in_my_graveyard(true, false, false, 25)]
#[case::on_my_battlefield(true, false, true, 25)]
#[case::only_in_their_graveyard(false, true, false, 20)]
#[case::nowhere(false, false, false, 20)]
fn a_desert_counts_where_it_is_yours(
    #[case] mine: bool,
    #[case] theirs: bool,
    #[case] battlefield: bool,
    #[case] life: i32,
) {
    assert_eq!(life_with_desert(mine, theirs, battlefield), life);
}

#[rstest]
#[case::you("AttackingPlayer$ Player | AttackedTarget$ You", json!("You"))]
#[case::enchanted(
    "AttackingPlayer$ Player | AttackedTarget$ Player.EnchantedBy",
    json!("AttachedPlayer")
)]
fn attackers_at_one_player_are_the_group_at_that_player(#[case] keys: &str, #[case] at: Value) {
    let text = trigger(&format!("AttackersDeclared | {keys}"));
    assert_eq!(gaps(&text), Vec::<String>::new());
    let one = trigger(&format!("AttackersDeclaredOneTarget | {keys}"));
    let found = &abilities(&text)[0]["Triggered"]["when"];
    assert_eq!(found, &abilities(&one)[0]["Triggered"]["when"]);
    assert_eq!(found["GroupAttacksAt"]["at"], at);
}

#[rstest]
#[case::any_player("AttackedTarget$ Player")]
#[case::opponents("AttackedTarget$ Opponent")]
#[case::player_or_walker("AttackedTarget$ You,Planeswalker.YouCtrl")]
fn several_attacked_players_stay_a_gap(#[case] keys: &str) {
    let text = trigger(&format!(
        "AttackersDeclared | AttackingPlayer$ Player | {keys}"
    ));
    assert!(
        gaps(&text)
            .iter()
            .any(|found| found.starts_with("key:Trigger.AttackersDeclared.AttackedTarget")),
        "{:?}",
        gaps(&text)
    );
}
