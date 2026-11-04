//! Goal: the trigger and selector rules of round 4 piece 4 turn Forge lines into the DSL nodes the engine has and leave the rest as gaps.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn trigger(line: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Elf\nT:{line} | TriggerZones$ Battlefield | Execute$ Go\nSVar:Go:DB$ Draw | NumCards$ 1\n"
    )
}

#[rstest]
#[case::mine("Mode$ AbilityCast | ValidActivatingPlayer$ You | ValidSA$ SpellAbility.!ManaAbility", json!({"Did": {"action": "Activated", "who": "You"}}))]
#[case::theirs("Mode$ AbilityCast | ValidActivatingPlayer$ Opponent | ValidSA$ Activated.!ManaAbility", json!({"Did": {"action": "Activated", "who": "Opponents"}}))]
#[case::anyones("Mode$ AbilityCast | ValidSA$ SpellAbility", json!({"Did": {"action": "Activated"}}))]
#[case::of_a_creature("Mode$ AbilityCast | ValidCard$ Creature.YouCtrl | ValidActivatingPlayer$ You", json!({"Did": {"action": "Activated", "who": "You", "what": [{"Type": "Creature"}, {"ControlledBy": "You"}]}}))]
#[case::spell_or_ability("Mode$ SpellAbilityCast | ValidSA$ Spell,Activated | ValidActivatingPlayer$ You", json!({"AnyOf": [{"SpellCast": {"by": "You"}}, {"Did": {"action": "Activated", "who": "You"}}]}))]
fn activations(#[case] line: &str, #[case] when: Value) {
    let expected = normal(json!({"Triggered": {"when": when, "effects": [{"Draw": {}}]}}));
    assert_eq!(abilities(&trigger(line)), vec![expected]);
}

#[rstest]
#[case::loyalty(
    "Mode$ AbilityCast | ValidActivatingPlayer$ You | ValidSA$ Activated.Loyalty",
    "key:Trigger.AbilityCast.ValidSA=Activated.Loyalty"
)]
#[case::tap_cost(
    "Mode$ AbilityCast | ValidSA$ Activated.!hasTapCost",
    "key:Trigger.AbilityCast.ValidSA=Activated.!hasTapCost"
)]
fn restricted_activations_stay_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&trigger(line)), vec![gap.to_owned()]);
}

#[rstest]
#[case::elves_at_an_opponent(
    "Mode$ AttackersDeclaredOneTarget | ValidAttackers$ Creature.Elf+YouCtrl | AttackedTarget$ Opponent",
    json!({"what": [{"Subtype": "Elf"}, {"ControlledBy": "You"}], "at": "Opponents"})
)]
#[case::two_or_more_at_a_player(
    "Mode$ AttackersDeclaredOneTarget | AttackingPlayer$ You | AttackedTarget$ Player | ValidAttackers$ Creature.YouCtrl | ValidAttackersAmount$ GE2",
    json!({"what": [{"Type": "Creature"}, {"ControlledBy": "You"}], "at": "EachPlayer", "at_least": 2})
)]
fn attackers_at_one_player(#[case] line: &str, #[case] node: Value) {
    let expected =
        normal(json!({"Triggered": {"when": {"GroupAttacksAt": node}, "effects": [{"Draw": {}}]}}));
    assert_eq!(abilities(&trigger(line)), vec![expected]);
}

#[rstest]
#[case::alone(
    "Mode$ AttackersDeclaredOneTarget | ValidAttackers$ Creature.YouCtrl | AttackedTarget$ Player | ValidAttackersAmount$ EQ1",
    "key:Trigger.AttackersDeclaredOneTarget.ValidAttackersAmount"
)]
#[case::or_a_planeswalker(
    "Mode$ AttackersDeclaredOneTarget | ValidAttackers$ Creature.YouCtrl | AttackedTarget$ Player,Planeswalker",
    "key:Trigger.AttackersDeclaredOneTarget.AttackedTarget=Player,Planeswalker"
)]
#[case::the_initiative(
    "Mode$ AttackersDeclaredOneTarget | ValidAttackers$ Creature.YouCtrl | AttackedTarget$ Player.hasInitiative",
    "key:Trigger.AttackersDeclaredOneTarget.AttackedTarget=Player.hasInitiative"
)]
fn attacks_that_are_not_on_one_player_stay_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&trigger(line)), vec![gap.to_owned()]);
}

fn creature_with(line: &str) -> String {
    format!("Name:E\nTypes:Artifact Equipment Elf\n{line}\n")
}

#[rstest]
#[case::flavor_name("K:Equip:3:Flavor Krishna", json!({"Keyword": {"Equip": "{3}"}}))]
#[case::bare_subtype("K:Equip:2:Elf.YouCtrl:Elf", json!({"Keyword": {"EquipTo": {"to": [{"Subtype": "Elf"}], "cost": "{2}"}}}))]
fn equip_labels_and_subtype_shorthand(#[case] line: &str, #[case] expected: Value) {
    assert_eq!(abilities(&creature_with(line)), vec![expected]);
}

#[rstest]
#[case::planeswalker("K:Equip:1:Planeswalker.YouCtrl:planeswalker")]
#[case::a_cost_change("K:Equip:3:::ReduceCost$ Y:This ability costs {1} less")]
#[case::three_types("K:Equip:1:Creature.YouCtrl+Shaman,Creature.YouCtrl+Wizard:Shaman or Wizard")]
fn other_equip_restrictions_stay_gaps(#[case] line: &str) {
    assert!(
        gaps(&creature_with(line))
            .iter()
            .any(|gap| gap.starts_with("keyword:Equip")),
        "{line}"
    );
}
