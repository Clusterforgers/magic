//! Goal: the verbs `RevealHand`, `NameCard`, `CopySpellAbility` and `ChoosePlayer` map to the node that runs the same rules, and
//! a shape the node cannot say stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line}\n")
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
#[case::reveal_remember(
    "RevealHand | ValidTgts$ Player | RememberRevealed$ True",
    "key:RevealHand.RememberRevealed"
)]
#[case::name_random("NameCard | Defined$ You | AtRandom$ True", "key:NameCard.AtRandom")]
#[case::name_list(
    "NameCard | Defined$ You | ChooseFromList$ Alpha,Beta",
    "key:NameCard.ChooseFromList"
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
