//! Goal: piece P7 of the rules plan translates its constructs, one rstest table per rule.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

/// The event pattern of a card whose only ability is the trigger line `trigger` that draws a card.
fn when(trigger: &str) -> Value {
    let text = format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nT:{trigger} | Execute$ Go\nSVar:Go:DB$ Draw | NumCards$ 1\n"
    );
    abilities(&text)
        .first()
        .and_then(|ability| ability.pointer("/Triggered/when"))
        .cloned()
        .unwrap_or(Value::Null)
}

fn gap_of(trigger: &str) -> Vec<String> {
    gaps(&format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nT:{trigger} | Execute$ Go\nSVar:Go:DB$ Draw | NumCards$ 1\n"
    ))
}

#[rstest]
#[case::enchanted_controller(
    "Mode$ Phase | Phase$ Upkeep | ValidPlayer$ Player.EnchantedController",
    json!({"StepBegins": {"step": "Upkeep", "whose": {"ControllerOf": "Attached"}}})
)]
#[case::enchanted_player(
    "Mode$ Phase | Phase$ Upkeep | ValidPlayer$ Player.EnchantedBy",
    json!({"StepBegins": {"step": "Upkeep", "whose": "AttachedPlayer"}})
)]
#[case::attackers_opponent(
    "Mode$ AttackersDeclared | AttackingPlayer$ Player.Opponent",
    json!({"GroupAttacks": {"what": [{"Type": "Creature"}, {"ControlledBy": "Opponents"}]}})
)]
#[case::attackers_anyone(
    "Mode$ AttackersDeclared | AttackingPlayer$ Player",
    json!({"GroupAttacks": {"what": [{"Type": "Creature"}]}})
)]
#[case::attackers_enchanted(
    "Mode$ AttackersDeclared | AttackingPlayer$ Player.EnchantedBy",
    json!({"GroupAttacks": {"what": [{"Type": "Creature"}, {"ControlledBy": "AttachedPlayer"}]}})
)]
#[case::attacks_you(
    "Mode$ Attacks | ValidCard$ Creature | Attacked$ You",
    json!({"AttacksPlayer": {"what": [{"Type": "Creature"}], "at": "You"}})
)]
#[case::attacks_you_or_walker(
    "Mode$ Attacks | ValidCard$ Creature | Attacked$ You,Planeswalker.YouCtrl",
    json!({"AttacksPlayerOrPlaneswalker": {"what": [{"Type": "Creature"}], "at": "You"}})
)]
#[case::attacks_enchanted(
    "Mode$ Attacks | ValidCard$ Creature | Attacked$ Player.EnchantedBy",
    json!({"AttacksPlayer": {"what": [{"Type": "Creature"}], "at": "AttachedPlayer"}})
)]
#[case::damage_dealt_once(
    "Mode$ DamageDealtOnce | ValidSource$ Card.Self | CombatDamage$ True",
    json!({"Batch": {"DealsDamage": {"source": ["This"], "combat": true}}})
)]
#[case::unlock_this_door(
    "Mode$ UnlockDoor | ValidPlayer$ You | ValidCard$ Card.Self | ThisDoor$ True",
    json!({"DoorUnlocked": {"who": "You", "door": 0}})
)]
#[case::crime("Mode$ CommitCrime | ValidPlayer$ You", json!({"W11cCrime": {"who": "You"}}))]
#[case::crime_opponent(
    "Mode$ CommitCrime | ValidPlayer$ Opponent",
    json!({"W11cCrime": {"who": "Opponents"}})
)]
#[case::dungeon(
    "Mode$ DungeonCompleted | ValidPlayer$ You",
    json!({"CompletedDungeon": {"who": "You"}})
)]
#[case::crewed(
    "Mode$ BecomesCrewed | ValidCard$ Card.Self",
    json!({"BecomesCrewed": {"what": ["This"]}})
)]
#[case::crewed_first(
    "Mode$ BecomesCrewed | ValidCard$ Card.Self | FirstTimeCrewed$ True",
    json!({"Nth": {"event": {"BecomesCrewed": {"what": ["This"]}}, "nth": 1}})
)]
#[case::attached_self(
    "Mode$ Attached | ValidSource$ Card.Self | ValidTarget$ Creature",
    json!({"Attached": {"what": ["This"], "to": [{"Type": "Creature"}]}})
)]
fn event_words_read_as_the_pattern_they_print(#[case] trigger: &str, #[case] expected: Value) {
    assert_eq!(when(trigger), expected);
}

#[rstest]
#[case::attacked_battle(
    "Mode$ Attacks | ValidCard$ Creature | Attacked$ Battle",
    "key:Trigger.Attacks.Attacked=Battle"
)]
#[case::attacked_planeswalker_of_another(
    "Mode$ Attacks | ValidCard$ Creature | Attacked$ You,Planeswalker.OppCtrl",
    "key:Trigger.Attacks.Attacked=You,Planeswalker.OppCtrl"
)]
#[case::attacking_player_unknown(
    "Mode$ AttackersDeclared | AttackingPlayer$ Player.withMostLife",
    "key:Trigger.AttackersDeclared.AttackingPlayer=Player.withMostLife"
)]
#[case::phase_player_unknown(
    "Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You.descended",
    "key:Trigger.Phase.ValidPlayer=You.descended"
)]
#[case::door_without_this_door(
    "Mode$ UnlockDoor | ValidPlayer$ You | ValidCard$ Card.Self",
    "key:Trigger.UnlockDoor.ThisDoor"
)]
#[case::crew_amount(
    "Mode$ BecomesCrewed | ValidCard$ Card.Self | ValidCrewAmount$ 2",
    "key:Trigger.BecomesCrewed.ValidCrewAmount"
)]
#[case::attached_static(
    "Mode$ Attached | ValidSource$ Card.Self | Static$ True",
    "key:Trigger.Attached.Static"
)]
fn a_word_the_table_does_not_know_stays_a_named_gap(#[case] trigger: &str, #[case] name: &str) {
    assert_eq!(gap_of(trigger), [name]);
}
