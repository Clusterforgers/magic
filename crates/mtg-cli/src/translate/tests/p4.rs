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
