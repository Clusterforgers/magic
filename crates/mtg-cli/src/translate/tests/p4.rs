//! Goal: piece P4 (ability header): keys shared by every API are read once, before the verb's mapper, and a value they do not know stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Instant\nA:{line}\n")
}

/// The first ability of a spell line.
fn first(line: &str) -> Value {
    abilities(&spell(line))
        .into_iter()
        .next()
        .unwrap_or_default()
}

#[test]
fn unknown_trigger_mode_is_a_gap() {
    let found = gaps(
        "Name:T\nTypes:Creature Elf\nT:Mode$ Nonsense | ValidCard$ Card.Self | Execute$ Go\nSVar:Go:DB$ Draw\n",
    );
    assert_eq!(found, ["trigger:Nonsense"]);
}

#[rstest]
#[case::graveyard_owned(
    "SP$ Pump | ValidTgts$ Creature.YouCtrl | TgtZone$ Graveyard | NumAtt$ +1",
    json!([{"Type": "Creature"}, {"OwnedBy": "You"}, {"In": "Graveyard"}])
)]
#[case::two_zones(
    "SP$ Pump | ValidTgts$ Creature | TgtZone$ Graveyard,Exile | NumAtt$ +1",
    json!([{"Type": "Creature"}, {"AnyOf": [{"In": "Graveyard"}, {"In": "Exile"}]}])
)]
#[case::spell_of_an_opponent(
    "SP$ Pump | ValidTgts$ Card | TgtZone$ Stack | TargetType$ Spell.OppCtrl | NumAtt$ +1",
    json!([{"ControlledBy": "Opponents"}, {"In": "Stack"}])
)]
fn a_target_zone_becomes_a_property_of_the_target(#[case] line: &str, #[case] selector: Value) {
    let found = first(line);
    assert_eq!(
        found["Spell"]["targets"][0]["what"],
        json!({"Object": selector})
    );
}

#[rstest]
#[case::ante("SP$ Pump | ValidTgts$ Creature | TgtZone$ Ante | NumAtt$ +1")]
#[case::permanent("SP$ Pump | ValidTgts$ Permanent | TgtZone$ Graveyard | NumAtt$ +1")]
#[case::player("SP$ Pump | ValidTgts$ Player | TgtZone$ Graveyard | NumAtt$ +1")]
#[case::not_controlled(
    "SP$ Pump | ValidTgts$ Creature.YouDontCtrl | TgtZone$ Graveyard | NumAtt$ +1"
)]
#[case::unknown_target_type(
    "SP$ Pump | ValidTgts$ Card | TgtZone$ Stack | TargetType$ Spell.singleTarget | NumAtt$ +1"
)]
#[case::mixed_with_open("SP$ Pump | ValidTgts$ Creature | TgtZone$ Battlefield,Exile | NumAtt$ +1")]
fn a_target_zone_the_selector_cannot_state_stays_a_gap(#[case] line: &str) {
    assert!(!gaps(&spell(line)).is_empty());
}

/// A spell that draws, then gains 2 life under the extra keys.
fn chain(extra: &str) -> String {
    format!(
        "Name:T\nTypes:Instant\nA:SP$ Draw | NumCards$ 1 | SubAbility$ B\nSVar:B:DB$ GainLife | LifeAmount$ 2 | {extra}\n"
    )
}

#[rstest]
#[case::your_turn("ConditionPlayerTurn$ True", json!("YourTurn"))]
#[case::not_your_turn("ConditionPlayerTurn$ False", json!({"Not": "YourTurn"}))]
#[case::mana_spent("ConditionManaSpent$ W", json!({"ManaSpent": "White"}))]
#[case::both_colors_spent(
    "ConditionManaSpent$ W U",
    json!({"All": [{"ManaSpent": "White"}, {"ManaSpent": "Blue"}]})
)]
#[case::mana_not_spent("ConditionManaNotSpent$ G", json!({"Not": {"ManaSpent": "Green"}}))]
#[case::fewer_life(
    "ConditionLifeTotal$ You | ConditionLifeAmount$ LT3",
    json!({"Compare": {"left": {"Life": "You"}, "op": "Lt", "right": 3}})
)]
#[case::none_present(
    "ConditionNotPresent$ Creature.token",
    json!({"Compare": {"left": {"Count": [{"Type": "Creature"}, "Token"]}, "op": "Eq", "right": 0}})
)]
#[case::second_clause(
    "ConditionPresent$ Artifact.YouCtrl | ConditionPresent2$ Enchantment.YouCtrl",
    json!({"All": [
        {"Compare": {"left": {"Count": [{"Type": "Artifact"}, {"ControlledBy": "You"}]}, "op": "Ge", "right": 1}},
        {"Compare": {"left": {"Count": [{"Type": "Enchantment"}, {"ControlledBy": "You"}]}, "op": "Ge", "right": 1}}]})
)]
#[case::kicked("Condition$ Kicked", json!("Kicked"))]
#[case::blessing("Condition$ Blessing", json!({"CitysBlessing": "You"}))]
#[case::monarch("Condition$ Monarch", json!({"Monarch": "You"}))]
#[case::night("Condition$ Night", json!("IsNight"))]
fn a_condition_key_of_a_chain_line_gates_the_effect(#[case] keys: &str, #[case] expected: Value) {
    let found = first_of(&chain(keys));
    assert_eq!(found["Spell"]["effects"][1]["If"]["condition"], expected);
}

fn first_of(text: &str) -> Value {
    abilities(text).into_iter().next().unwrap_or_default()
}

/// A trigger of the card entering, under extra keys.
fn trigger(extra: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Elf\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Go | {extra}\nSVar:Go:DB$ Draw\n"
    )
}

#[rstest]
#[case::your_turn("PlayerTurn$ True", json!({"AtTrigger": "YourTurn"}))]
#[case::opponents_turn("OpponentTurn$ True", json!({"AtTrigger": {"Not": "YourTurn"}}))]
#[case::first_combat("FirstCombat$ True", json!("FirstCombat"))]
#[case::delirium(
    "Delirium$ True",
    json!({"Compare": {"left": {"CardTypes": [{"In": "Graveyard"}, {"OwnedBy": "You"}]}, "op": "Ge", "right": 4}})
)]
fn a_gating_word_of_a_trigger_is_its_if(#[case] keys: &str, #[case] expected: Value) {
    let found = first_of(&trigger(keys));
    assert_eq!(found["Triggered"]["if"], expected);
}

#[rstest]
#[case::revolt("Revolt$ True")]
#[case::not_true("PlayerTurn$ False")]
fn a_gating_word_the_table_does_not_know_stays_a_gap(#[case] keys: &str) {
    assert!(!gaps(&trigger(keys)).is_empty());
}

#[rstest]
#[case::turn_word("ConditionPlayerTurn$ Maybe")]
#[case::two_colors_not_spent("ConditionManaNotSpent$ R G")]
#[case::other_player_life("ConditionLifeTotal$ TargetedOrController | ConditionLifeAmount$ GT3")]
#[case::life_without_amount("ConditionLifeTotal$ You")]
#[case::unknown_word("Condition$ MaxSpeed")]
#[case::not_present_beside_present("ConditionNotPresent$ Card | ConditionPresent$ Land")]
fn a_condition_value_the_table_does_not_know_stays_a_gap(#[case] keys: &str) {
    assert!(!gaps(&chain(keys)).is_empty());
}
