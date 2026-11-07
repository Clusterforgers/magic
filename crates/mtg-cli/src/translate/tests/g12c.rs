//! Goal: a continuous line that adds `Legendary` or `Snow` gives that supertype, one half of base power and toughness
//! outside a characteristic-defining line sets that half only, a defining `1+*` half reads its count when the count adds
//! the printed number, and a cost modifier of loyalty abilities, of any player's spells, of the enchanted player's spells,
//! of spells without `Type$` and of spells and abilities together is the modifier it states; any other shape stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn statics(script: &str) -> Vec<Value> {
    abilities(script)
}

#[rstest]
#[case::legendary(
    "Name:T\nTypes:Enchantment Aura\nK:Enchant:Permanent\nS:Mode$ Continuous | AffectedDefined$ Enchanted | Affected$ Permanent | AddType$ Legendary | Description$ Enchanted permanent is legendary.\n",
    json!({"Static": {"affected": "Attached", "mods": [{"AddSupertype": "Legendary"}]}})
)]
#[case::snow_and_a_subtype(
    "Name:T\nTypes:Enchantment\nS:Mode$ Continuous | Affected$ Land.YouCtrl | AddType$ Snow & Island | Description$ x\n",
    json!({"Static": {"affected": {"Each": [{"Type": "Land"}, {"ControlledBy": "You"}]}, "mods": [{"AddSupertype": "Snow"}, {"AddSubtype": "Island"}]}})
)]
#[case::base_power(
    "Name:T\nTypes:Enchantment Aura\nK:Enchant:Creature\nS:Mode$ Continuous | AffectedDefined$ Enchanted | SetPower$ 0 | Description$ x\n",
    json!({"Static": {"affected": "Attached", "mods": [{"SetPower": 0}]}})
)]
#[case::base_toughness(
    "Name:T\nTypes:Creature Bird\nPT:5/5\nS:Mode$ Continuous | Affected$ Creature.OppCtrl | SetToughness$ 1 | Description$ x\n",
    json!({"Static": {"affected": {"Each": [{"Type": "Creature"}, {"ControlledBy": "Opponents"}]}, "mods": [{"SetToughness": 1}]}})
)]
fn a_supertype_and_one_base_half_are_their_modifications(
    #[case] script: &str,
    #[case] expected: Value,
) {
    assert!(
        statics(script).contains(&expected),
        "{:?} {:?}",
        statics(script),
        gaps(script)
    );
}

#[rstest]
#[case::plus_one(
    "PT:1+*/3",
    "Count$Valid Creature.Green/Plus.1",
    json!({"Sum": [{"Count": [{"Type": "Creature"}, {"Color": "Green"}]}, 1]})
)]
fn a_defining_half_with_a_printed_number_reads_the_count(
    #[case] pt: &str,
    #[case] count: &str,
    #[case] power: Value,
) {
    let script = format!(
        "Name:T\nTypes:Creature Spirit\n{pt}\nS:Mode$ Continuous | CharacteristicDefining$ True | SetPower$ X | Description$ x\nSVar:X:{count}\n"
    );
    let found = statics(&script);
    let set = found
        .iter()
        .find_map(|ability| ability["Static"]["mods"][0].get("SetPT").cloned());
    assert_eq!(
        set,
        Some(json!({"power": power, "toughness": 3})),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::count_without_the_plus("PT:1+*/3", "Count$Valid Creature.Green")]
#[case::other_plus("PT:2+*/3", "Count$Valid Creature.Green/Plus.1")]
fn a_defining_half_whose_count_does_not_add_the_number_is_a_gap(
    #[case] pt: &str,
    #[case] count: &str,
) {
    let script = format!(
        "Name:T\nTypes:Creature Spirit\n{pt}\nS:Mode$ Continuous | CharacteristicDefining$ True | SetPower$ X | Description$ x\nSVar:X:{count}\n"
    );
    assert!(gaps(&script).contains(&"key:Static.Continuous.half-SetPT".to_owned()));
}

const PERMANENT: &str = "Name:T\nTypes:Artifact\n";

#[rstest]
#[case::loyalty(
    "RaiseCost | ValidCard$ Planeswalker.OppCtrl | ValidSpell$ Activated.Loyalty | Amount$ 1",
    json!([{"CostModifier": {"activating": "Loyalty", "applies_to": [{"Type": "Planeswalker"}, {"ControlledBy": "Opponents"}], "change": "Increase", "mana": "{1}"}}])
)]
#[case::any_player(
    "ReduceCost | ValidTarget$ Card.Self | Activator$ Player | Type$ Spell | Amount$ 2",
    json!([{"CostModifier": {"applies_to": ["Spell", {"Targeting": ["This"]}], "change": "Reduce", "mana": "{2}"}}])
)]
#[case::enchanted_player(
    "RaiseCost | ValidCard$ Card.nonCreature | Type$ Spell | Activator$ Player.EnchantedBy | Amount$ 2",
    json!([{"CostModifier": {"applies_to": ["Spell", {"Not": {"Type": "Creature"}}, {"ControlledBy": "AttachedPlayer"}], "change": "Increase", "mana": "{2}"}}])
)]
#[case::not_on_its_casters_turn(
    "RaiseCost | Activator$ Player.NonActive | Type$ Spell | Amount$ 3",
    json!([{"CostModifier": {"applies_to": ["Spell", {"Not": {"ControlledBy": "ActivePlayer"}}], "change": "Increase", "mana": "{3}"}}])
)]
#[case::spells_without_type(
    "RaiseCost | ValidCard$ Card | Activator$ Opponent | ValidSpell$ Spell | Amount$ 1",
    json!([{"CostModifier": {"applies_to": ["Spell", {"ControlledBy": "Opponents"}], "change": "Increase", "mana": "{1}"}}])
)]
#[case::spells_and_abilities(
    "RaiseCost | ValidCard$ Card | Activator$ Opponent | ValidSpell$ Spell,Activated.!ManaAbility | Amount$ 1",
    json!([
        {"CostModifier": {"applies_to": ["Spell", {"ControlledBy": "Opponents"}], "change": "Increase", "mana": "{1}"}},
        {"CostModifier": {"activating": "Any", "applies_to": [{"ControlledBy": "Opponents"}], "change": "Increase", "mana": "{1}"}}
    ])
)]
fn a_cost_modifier_line_is_the_modifier_it_states(#[case] line: &str, #[case] expected: Value) {
    let script = format!("{PERMANENT}S:Mode$ {line} | Description$ x\n");
    assert_eq!(
        Value::Array(statics(&script)),
        expected,
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::other_kind("RaiseCost | ValidCard$ Card | ValidSpell$ Activated.Boast | Amount$ 1")]
#[case::other_player("RaiseCost | Type$ Spell | Activator$ Player.Chosen | Amount$ 3")]
#[case::spells_and_abilities_with_type(
    "RaiseCost | ValidCard$ Card | Type$ Spell | ValidSpell$ Spell,Activated.!ManaAbility | Amount$ 1"
)]
fn another_cost_modifier_shape_stays_a_gap(#[case] line: &str) {
    let script = format!("{PERMANENT}S:Mode$ {line} | Description$ x\n");
    assert!(!gaps(&script).is_empty(), "{:?}", statics(&script));
}
