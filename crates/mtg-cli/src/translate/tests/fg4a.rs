//! Goal: the selector atoms, damage prevention and source memory of the fg4a family map invented Forge-format lines to
//! the DSL nodes the engine has, and every shape outside them stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

#[rstest]
#[case::cast_by_you(
    "Name:T\nTypes:Creature Elf\nS:Mode$ Continuous | Affected$ Creature.wasCastByYou | AddPower$ 1\n",
    json!({"Static": {"affected": {"Each": [{"Type": "Creature"}, {"WasCast": {"by": "You"}}]}, "mods": [{"ModifyPT": {"power": 1, "toughness": 0}}]}})
)]
#[case::cast_from_your_hand(
    "Name:T\nTypes:Creature Elf\nS:Mode$ Continuous | Affected$ Creature.wasCastFromYourHand | AddPower$ 1\n",
    json!({"Static": {"affected": {"Each": [{"Type": "Creature"}, {"WasCast": {"from": "Hand"}}, {"OwnedBy": "You"}]}, "mods": [{"ModifyPT": {"power": 1, "toughness": 0}}]}})
)]
#[case::blocking_the_source(
    "Name:T\nTypes:Creature Elf\nS:Mode$ Continuous | Affected$ Creature.blockingSource | AddToughness$ 1\n",
    json!({"Static": {"affected": {"Each": [{"Type": "Creature"}, {"Blocks": "This"}]}, "mods": [{"ModifyPT": {"power": 0, "toughness": 1}}]}})
)]
#[case::damaged_this_turn(
    "Name:T\nTypes:Creature Elf\nS:Mode$ Continuous | Affected$ Creature.wasDealtDamageThisTurn | AddPower$ 1\n",
    json!({"Static": {"affected": {"Each": [{"Type": "Creature"}, "DamagedThisTurn"]}, "mods": [{"ModifyPT": {"power": 1, "toughness": 0}}]}})
)]
#[case::cards_in_hand_get_a_keyword(
    "Name:T\nTypes:Enchantment\nS:Mode$ Continuous | Affected$ Creature.YouOwn | AffectedZone$ Hand | AddKeyword$ Flash\n",
    json!({"Static": {"affected": {"Each": [{"Type": "Creature"}, {"OwnedBy": "You"}, {"In": "Hand"}]}, "mods": [{"AddKeyword": "Flash"}]}})
)]
#[case::spells_on_the_stack_get_a_keyword(
    "Name:T\nTypes:Enchantment\nS:Mode$ Continuous | Affected$ Creature.YouCtrl+wasCastFromHand | AffectedZone$ Stack | AddKeyword$ Flash\n",
    json!({"Static": {"affected": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}, {"WasCast": {"from": "Hand"}}, {"In": "Stack"}]}, "mods": [{"AddKeyword": "Flash"}]}})
)]
fn selector_atoms_map(#[case] script: &str, #[case] expected: Value) {
    let mut found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    assert_eq!(normal(found.remove(0)), normal(expected));
}

#[rstest]
#[case::wrong_cast_zone("Creature.wasCastFromTheirHand")]
#[case::unknown_cast_suffix("Creature.wasCastFromNowhere")]
#[case::every_zone_at_once("Creature.YouOwn | AffectedZone$ All")]
#[case::saddled_is_a_condition("Creature.IsSaddled")]
fn atoms_outside_the_nodes_stay_gaps(#[case] valid: &str) {
    let script = format!(
        "Name:T\nTypes:Creature Elf\nS:Mode$ Continuous | Affected$ {valid} | AddPower$ 1\n"
    );
    assert!(abilities(&script).is_empty());
    assert!(!gaps(&script).is_empty());
}

#[rstest]
#[case::static_self_combat(
    "Name:T\nTypes:Creature Elf\nR:Event$ DamageDone | Prevent$ True | IsCombat$ True | ValidTarget$ Card.Self | Description$ x\n",
    json!({"Replacement": {"when": {"Damage": {"to": "This", "combat": true}}, "by": "Prevent"}})
)]
#[case::static_from_enchanted(
    "Name:T\nTypes:Enchantment Aura\nR:Event$ DamageDone | ActiveZones$ Battlefield | Prevent$ True | ValidSource$ Creature.EnchantedBy\n",
    json!({"Replacement": {"when": {"Damage": {"source": [{"Type": "Creature"}, {"Is": "Attached"}]}}, "by": "Prevent"}})
)]
#[case::shield_to_target(
    "Name:T\nTypes:Instant\nA:SP$ Effect | ValidTgts$ Creature | ReplacementEffects$ RPrevent | RememberObjects$ Targeted\nSVar:RPrevent:Event$ DamageDone | Prevent$ True | ValidTarget$ Card.IsRemembered | Description$ x\n",
    json!({"Spell": {"targets": [{"what": {"Object": [{"Type": "Creature"}]}}],
        "effects": [{"Shield": {"when": {"Damage": {"to": {"Target": 0}}}, "by": "Prevent", "duration": "UntilEndOfTurn"}}]}})
)]
#[case::shield_all_combat(
    "Name:T\nTypes:Instant\nA:SP$ Effect | ReplacementEffects$ RPrevent\nSVar:RPrevent:Event$ DamageDone | Prevent$ True | IsCombat$ True | ActiveZones$ Command\n",
    json!({"Spell": {"effects": [{"Shield": {"when": {"Damage": {"combat": true}}, "by": "Prevent", "duration": "UntilEndOfTurn"}}]}})
)]
#[case::shield_from_target(
    "Name:T\nTypes:Instant\nA:SP$ Effect | ValidTgts$ Creature | ReplacementEffects$ RPrevent | RememberObjects$ Targeted\nSVar:RPrevent:Event$ DamageDone | Prevent$ True | ValidSource$ Card.IsRemembered\n",
    json!({"Spell": {"targets": [{"what": {"Object": [{"Type": "Creature"}]}}],
        "effects": [{"Shield": {"when": {"Damage": {"source": [{"Is": {"Target": 0}}]}}, "by": "Prevent", "duration": "UntilEndOfTurn"}}]}})
)]
fn damage_prevention_maps(#[case] script: &str, #[case] expected: Value) {
    let mut found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    assert_eq!(normal(found.remove(0)), normal(expected));
}

#[rstest]
#[case::replaced_not_prevented(
    "R:Event$ DamageDone | ValidTarget$ You | ReplaceWith$ Dmg\nSVar:Dmg:DB$ ReplaceDamage | Amount$ 1\n"
)]
#[case::other_zone("R:Event$ DamageDone | Prevent$ True | ActiveZones$ Graveyard | ValidTarget$ You\n")]
#[case::unread_key("R:Event$ DamageDone | Prevent$ True | ValidTarget$ You | Quux$ True\n")]
#[case::remembered_in_a_static("R:Event$ DamageDone | Prevent$ True | ValidSource$ Card.IsRemembered\n")]
fn other_damage_replacements_stay_gaps(#[case] line: &str) {
    let script = format!("Name:T\nTypes:Enchantment\n{line}");
    assert!(abilities(&script).is_empty(), "{script}");
    assert!(!gaps(&script).is_empty());
}
