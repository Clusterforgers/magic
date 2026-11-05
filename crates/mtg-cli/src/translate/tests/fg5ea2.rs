//! Goal: the verbs `RevealHand`, `NameCard`, `CopySpellAbility` and `ChoosePlayer` map to the node that runs the same rules, and
//! a shape the node cannot say stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line}\n")
}

#[test]
fn the_chosen_player_is_read_by_the_next_line() {
    let script = spell(
        "ChoosePlayer | Defined$ You | Choices$ Player.Opponent | SubAbility$ Draw\nSVar:Draw:DB$ Draw | Defined$ ChosenPlayer\n",
    );
    let found = abilities(&script);
    assert_eq!(
        found[0]["Spell"]["effects"],
        json!([
            {"Choose": {"from": "Opponents", "as": "player"}},
            {"Draw": {"who": {"Bound": "player"}}}
        ]),
        "{:?}",
        gaps(&script)
    );
}

#[test]
fn a_cast_trigger_copies_the_spell_it_is_about() {
    let script = "Name:T\nTypes:Creature Wizard\nPT:1/1\nT:Mode$ SpellCast | ValidCard$ Instant,Sorcery | ValidActivatingPlayer$ You | Execute$ Copy | TriggerZones$ Battlefield | TriggerDescription$ x\nSVar:Copy:DB$ CopySpellAbility | Defined$ TriggeredSpellAbility | MayChooseTarget$ True\n";
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    assert!(
        found[0]
            .to_string()
            .contains(r#"{"CopySpell":{"new_targets":true,"what":"Triggering"}}"#),
        "{}",
        found[0]
    );
}

#[rstest]
#[case::reveal_target(
    "RevealHand | ValidTgts$ Player",
    json!({"Reveal": {"what": {"Each": [{"OwnedBy": {"Target": 0}}, {"In": "Hand"}]}}})
)]
#[case::reveal_opponent(
    "RevealHand | ValidTgts$ Opponent",
    json!({"Reveal": {"what": {"Each": [{"OwnedBy": {"Target": 0}}, {"In": "Hand"}]}}})
)]
#[case::reveal_each(
    "RevealHand | Defined$ Player",
    json!({"Reveal": {"what": {"Each": [{"OwnedBy": "EachPlayer"}, {"In": "Hand"}]}}})
)]
#[case::name_any(
    "NameCard | Defined$ You",
    json!({"ChooseValue": {"kind": {"CardName": []}, "as": "name"}})
)]
#[case::name_nonland(
    "NameCard | Defined$ You | ValidCards$ Card.nonLand | ValidDescription$ nonland",
    json!({"ChooseValue": {"kind": {"CardName": [{"Not": {"Type": "Land"}}]}, "as": "name"}})
)]
#[case::name_creature(
    "NameCard | ValidCards$ Creature",
    json!({"ChooseValue": {"kind": {"CardName": [{"Type": "Creature"}]}, "as": "name"}})
)]
#[case::copy_target(
    "CopySpellAbility | ValidTgts$ Instant,Sorcery | TargetType$ Spell | MayChooseTarget$ True",
    json!({"CopySpell": {"what": {"Target": 0}, "new_targets": true}})
)]
#[case::copy_twice(
    "CopySpellAbility | ValidTgts$ Instant.YouCtrl | TargetType$ Spell | Amount$ 2",
    json!({"CopySpell": {"what": {"Target": 0}, "count": 2}})
)]
#[case::copy_red(
    "CopySpellAbility | ValidTgts$ Instant | TargetType$ Spell | SetColor$ Red | MayChooseTarget$ True",
    json!({"CopySpell": {"what": {"Target": 0}, "new_targets": true, "except": [{"SetColors": ["Red"]}]}})
)]
#[case::choose_opponent(
    "ChoosePlayer | Defined$ You | Choices$ Player.Opponent",
    json!({"Choose": {"from": "Opponents", "as": "player"}})
)]
fn a_verb_maps_to_its_node(#[case] line: &str, #[case] node: Value) {
    let script = spell(line);
    let mut found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let found = found.remove(0);
    let targets = found["Spell"].get("targets").cloned();
    let mut want = json!({"Spell": {"effects": [node]}});
    if let Some(targets) = targets {
        want["Spell"]["targets"] = targets;
    }
    assert_eq!(normal(found), normal(want));
}

#[rstest]
#[case::reveal_look("RevealHand | ValidTgts$ Player | Look$ True", "key:RevealHand.Look")]
#[case::name_random("NameCard | Defined$ You | AtRandom$ True", "key:NameCard.AtRandom")]
#[case::name_list(
    "NameCard | Defined$ You | ChooseFromList$ Alpha,Beta",
    "key:NameCard.ChooseFromList"
)]
#[case::copy_ability(
    "CopySpellAbility | ValidTgts$ Card | TargetType$ Activated | MayChooseTarget$ True",
    "key:CopySpellAbility.TargetType"
)]
#[case::copy_other_controller(
    "CopySpellAbility | ValidTgts$ Instant | TargetType$ Spell | Controller$ TargetedController",
    "key:CopySpellAbility.Controller"
)]
#[case::copy_triggered_outside_a_cast(
    "CopySpellAbility | Defined$ TriggeredSpellAbility",
    "key:CopySpellAbility.Defined"
)]
#[case::choose_player_random(
    "ChoosePlayer | Defined$ You | Choices$ Player | Random$ True",
    "key:ChoosePlayer.Random"
)]
#[case::choose_targeted_player(
    "ChoosePlayer | Defined$ You | Choices$ ThisTargetedPlayer | ValidTgts$ Player",
    "key:ChoosePlayer.ValidTgts"
)]
fn a_form_beyond_the_node_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    let script = spell(line);
    assert_eq!(abilities(&script), Vec::<Value>::new(), "{script}");
    assert!(
        gaps(&script).iter().any(|found| found == gap),
        "{:?}",
        gaps(&script)
    );
}
