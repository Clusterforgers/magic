//! Goal: a prevention keyword given by a pump is a shield for the turn of the damage dealt to, by or to and by the
//! creatures as they are then, given by an Aura or Equipment it is a replacement of that permanent, and protection
//! from each color or from everything is the keyword; shapes the nodes cannot say stay named gaps.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

const TO: &str = "Prevent all damage that would be dealt to CARDNAME.";
const COMBAT_TO: &str = "Prevent all combat damage that would be dealt to CARDNAME.";
const BY: &str = "Prevent all damage that would be dealt by CARDNAME.";
const COMBAT_BY: &str = "Prevent all combat damage that would be dealt by CARDNAME.";
const BOTH: &str = "Prevent all combat damage that would be dealt to and dealt by CARDNAME.";

fn shield(damage: Value) -> Value {
    json!({"Shield": {"when": {"Damage": damage}, "by": "Prevent", "duration": "UntilEndOfTurn"}})
}

fn replacement(damage: Value) -> Value {
    json!({"Replacement": {"when": {"Damage": damage}, "by": "Prevent"}})
}

#[rstest]
#[case::to(TO, vec![shield(json!({"to": {"Target": 0}}))])]
#[case::combat_to(COMBAT_TO, vec![shield(json!({"to": {"Target": 0}, "combat": true}))])]
#[case::by(BY, vec![shield(json!({"source": [{"Is": {"Target": 0}}]}))])]
#[case::combat_by(COMBAT_BY, vec![shield(json!({"source": [{"Is": {"Target": 0}}], "combat": true}))])]
#[case::to_and_by(BOTH, vec![
    shield(json!({"to": {"Target": 0}, "combat": true})),
    shield(json!({"source": [{"Is": {"Target": 0}}], "combat": true})),
])]
fn a_pumped_prevention_is_a_shield_for_the_turn(#[case] word: &str, #[case] expected: Vec<Value>) {
    let script = format!("Name:T\nTypes:Instant\nA:SP$ Pump | ValidTgts$ Creature | KW$ {word}\n");
    let found = abilities(&script);
    assert_eq!(
        found[0]["Spell"]["effects"],
        json!(expected),
        "{:?}",
        gaps(&script)
    );
}

#[test]
fn a_pumped_prevention_beside_a_change_follows_the_change() {
    let script = format!(
        "Name:T\nTypes:Instant\nA:SP$ Pump | ValidTgts$ Creature | NumDef$ +2 | KW$ Flying & {COMBAT_BY}\n"
    );
    let effects = &abilities(&script)[0]["Spell"]["effects"];
    assert_eq!(
        effects[0]["Continuous"]["mods"],
        json!([{"ModifyPT": {"power": 0, "toughness": 2}}, {"AddKeyword": "Flying"}])
    );
    assert_eq!(
        effects[1],
        shield(json!({"source": [{"Is": {"Target": 0}}], "combat": true}))
    );
}

#[rstest]
#[case::longer("Duration$ UntilYourNextTurn", "keyword:Prevent-all-damage-for-longer")]
fn a_pumped_prevention_for_longer_stays_a_gap(#[case] extra: &str, #[case] gap: &str) {
    let script =
        format!("Name:T\nTypes:Instant\nA:SP$ Pump | ValidTgts$ Creature | {extra} | KW$ {BY}\n");
    assert!(
        gaps(&script).iter().any(|found| found == gap),
        "{:?}",
        gaps(&script)
    );
}

fn aura(line: &str) -> String {
    format!(
        "Name:T\nTypes:Enchantment Aura\nK:Enchant:Creature\nS:Mode$ Continuous | AffectedDefined$ Enchanted | Affected$ Creature | {line}\n"
    )
}

#[rstest]
#[case::to(COMBAT_TO, json!({"to": "Attached", "combat": true}))]
#[case::by(BY, json!({"source": [{"Is": "Attached"}]}))]
fn a_given_prevention_is_a_replacement_of_the_aura(#[case] word: &str, #[case] damage: Value) {
    let script = aura(&format!("AddKeyword$ {word}"));
    assert_eq!(
        abilities(&script)[1..],
        [replacement(damage)],
        "{:?}",
        gaps(&script)
    );
}

#[test]
fn a_given_prevention_beside_a_keyword_is_a_second_ability() {
    let script = aura(&format!("AddKeyword$ Shadow & {BY}"));
    assert_eq!(
        abilities(&script)[1..],
        [
            json!({"Static": {"affected": "Attached", "mods": [{"AddKeyword": "Shadow"}]}}),
            replacement(json!({"source": [{"Is": "Attached"}]}))
        ],
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::to_and_by("Creature", &format!("AddKeyword$ {BOTH}"), "key:Static.Continuous.prevention-to-and-by")]
#[case::refined(
    "Creature.Black",
    &format!("AddKeyword$ {TO}"),
    "key:Static.Continuous.prevention-shape"
)]
#[case::from_the_graveyard(
    "Creature",
    &format!("AddKeyword$ {TO} | EffectZone$ Graveyard"),
    "key:Static.Continuous.prevention-shape"
)]
fn a_given_prevention_the_nodes_cannot_say_stays_a_gap(
    #[case] affected: &str,
    #[case] line: &str,
    #[case] gap: &str,
) {
    let script = aura(line).replace("Affected$ Creature |", &format!("Affected$ {affected} |"));
    assert!(
        gaps(&script).iter().any(|found| found == gap),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::each_color("Protection from each color", json!({"Protection": [{"Not": "Colorless"}]}))]
#[case::everything("Protection from everything", json!({"Protection": []}))]
fn protection_from_each_color_or_everything_is_the_keyword(
    #[case] line: &str,
    #[case] expected: Value,
) {
    let script = format!("Name:T\nTypes:Creature Angel\nK:{line}\n");
    assert_eq!(abilities(&script), vec![json!({ "Keyword": expected })]);
}
