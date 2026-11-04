//! Goal: the `Remember*` keys of a step that moves objects name what it moved, and the lines after it read that name as `Remembered`.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn spell(first: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ {first} | SubAbility$ Back\nSVar:Back:DB$ Pump | Defined$ Remembered | NumAtt$ +1 | NumDef$ +1\n"
    )
}

fn pumped(name: &str) -> Value {
    json!({"Continuous": {"affected": {"Bound": name},
        "mods": [{"ModifyPT": {"power": 1, "toughness": 1}}], "duration": "UntilEndOfTurn"}})
}

#[rstest]
#[case::milled("Mill | NumCards$ 2 | RememberMilled$ True", json!({"Mill": {"count": 2, "as": "milled"}}), "milled")]
#[case::sacrificed(
    "Sacrifice | SacValid$ Creature | RememberSacrificed$ True",
    json!({"Sacrifice": {"filter": [{"Type": "Creature"}], "count": 1, "as": "sacrificed"}}),
    "sacrificed"
)]
#[case::destroyed_all(
    "DestroyAll | ValidCards$ Creature | RememberDestroyed$ True",
    json!({"Destroy": {"what": {"Each": [{"Type": "Creature"}]}, "regenerate": true, "as": "destroyed"}}),
    "destroyed"
)]
#[case::changed_all(
    "ChangeZoneAll | ChangeType$ Creature | Origin$ Battlefield | Destination$ Hand | RememberChanged$ True",
    json!({"Move": {"what": {"Each": [{"Type": "Creature"}]}, "to": "Hand", "as": "moved"}}),
    "moved"
)]
fn the_step_names_what_it_moved(#[case] first: &str, #[case] step: Value, #[case] name: &str) {
    let expected = normal(json!({"Spell": {"effects": [step, pumped(name)]}}));
    assert_eq!(abilities(&spell(first)), vec![expected]);
}

#[rstest]
#[case::changed_move(
    "Origin$ Graveyard | Destination$ Hand | RememberChanged$ True",
    "Hand",
    "moved"
)]
#[case::lki_move(
    "Origin$ Graveyard | Destination$ Hand | RememberLKI$ True",
    "Hand",
    "moved"
)]
#[case::targets_move(
    "Origin$ Graveyard | Destination$ Hand | RememberTargets$ True",
    "Hand",
    "moved"
)]
fn a_targeted_move_names_the_object(#[case] keys: &str, #[case] to: &str, #[case] name: &str) {
    let first = format!("ChangeZone | ValidTgts$ Creature | {keys}");
    let expected = normal(json!({"Spell": {
        "targets": [{"what": {"Object": [{"Type": "Creature"}, {"In": "Graveyard"}]}}],
        "effects": [{"Move": {"what": {"Target": 0}, "to": to, "as": name}}, pumped(name)]}}));
    assert_eq!(abilities(&spell(&first)), vec![expected]);
}

#[test]
fn remembered_targets_of_a_step_that_does_not_move_them_are_the_target() {
    let text = "Name:T\nTypes:Sorcery\nA:SP$ Tap | ValidTgts$ Creature | RememberTargets$ True | SubAbility$ Back\nSVar:Back:DB$ Pump | Defined$ Remembered | NumAtt$ +1 | NumDef$ +1\n";
    let expected = normal(json!({"Spell": {
        "targets": [{"what": {"Object": [{"Type": "Creature"}]}}],
        "effects": [{"Tap": {"what": {"Target": 0}}},
            {"Continuous": {"affected": {"Target": 0},
                "mods": [{"ModifyPT": {"power": 1, "toughness": 1}}], "duration": "UntilEndOfTurn"}}]}}));
    assert_eq!(abilities(text), vec![expected]);
}

#[test]
fn remembered_targets_without_a_target_stay_a_gap() {
    let text = "Name:T\nTypes:Sorcery\nA:SP$ Draw | NumCards$ 1 | RememberTargets$ True\n";
    assert_eq!(
        gaps(text),
        vec!["defined:Targeted-without-target".to_owned()]
    );
}
