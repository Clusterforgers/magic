//! Goal: a damage trigger keeps the kind of damage the script names (combat, noncombat or either), so "noncombat damage" is never any damage.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

#[rstest]
#[case::noncombat(" | CombatDamage$ False", Some(json!(false)))]
#[case::combat(" | CombatDamage$ True", Some(json!(true)))]
#[case::either("", None)]
fn a_damage_trigger_keeps_the_kind_of_damage(#[case] key: &str, #[case] combat: Option<Value>) {
    let text = format!(
        "Name:Subject\nManaCost:2\nTypes:Creature Elf\nPT:2/2\nT:Mode$ DamageDone | ValidSource$ Card.YouCtrl | ValidTarget$ Opponent{key} | TriggerZones$ Battlefield | Execute$ Pump\nSVar:Pump:DB$ Pump | Defined$ Self | NumAtt$ +1\n"
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = abilities(&text);
    let pattern = &found[0]["Triggered"]["when"]["DealsDamage"];
    assert_eq!(pattern.get("combat").cloned(), combat, "{found:?}");
}

#[test]
fn a_kind_of_damage_the_trigger_cannot_say_is_a_gap() {
    let text = "Name:Subject\nTypes:Creature Elf\nPT:2/2\nT:Mode$ DamageDone | ValidSource$ Card | CombatDamage$ Maybe | Execute$ Pump\nSVar:Pump:DB$ Pump | Defined$ Self | NumAtt$ +1\n";
    assert_eq!(
        gaps(text),
        vec!["key:Trigger.DamageDone.CombatDamage=Maybe".to_string()]
    );
}
