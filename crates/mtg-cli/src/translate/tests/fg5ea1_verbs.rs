//! Goal: `GenericChoice`, `Branch` and `SetState` map to the node that runs the same rules, and a shape beyond it stays a
//! named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn spell(line: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ {line}\nSVar:Win:DB$ GainLife | LifeAmount$ 2\nSVar:Lose:DB$ LoseLife | LifeAmount$ 2\nSVar:Third:DB$ Draw | NumCards$ 1\nSVar:N:Count$xPaid\n"
    )
}

#[rstest]
#[case::choice(
    "GenericChoice | Defined$ You | Choices$ Win,Lose",
    json!({"Either": {"first": [{"GainLife": {"amount": 2}}], "second": [{"LoseLife": {"amount": 2}}]}})
)]
#[case::choice_default_chooser(
    "GenericChoice | Choices$ Win,Lose",
    json!({"Either": {"first": [{"GainLife": {"amount": 2}}], "second": [{"LoseLife": {"amount": 2}}]}})
)]
#[case::branch(
    "Branch | BranchConditionSVar$ N | BranchConditionSVarCompare$ GE3 | TrueSubAbility$ Win | FalseSubAbility$ Lose",
    json!({"If": {
        "condition": {"Compare": {"left": "X", "op": "Ge", "right": 3}},
        "then": [{"GainLife": {"amount": 2}}],
        "otherwise": [{"LoseLife": {"amount": 2}}],
    }})
)]
#[case::transform("SetState | Defined$ Self | Mode$ Transform", json!({"Transform": {"what": "This"}}))]
#[case::transform_default("SetState | Mode$ Transform", json!({"Transform": {"what": "This"}}))]
fn a_verb_maps_to_its_node(#[case] line: &str, #[case] node: Value) {
    let script = spell(line);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(
        normal(found[0].clone()),
        normal(json!({"Spell": {"effects": [node]}}))
    );
}

#[rstest]
#[case::nine_choices(
    "GenericChoice | Choices$ Win,Lose,Third,Win,Lose,Third,Win,Lose,Third",
    "key:GenericChoice.Choices"
)]
#[case::everyone_chooses(
    "GenericChoice | Defined$ Player.Opponent | Choices$ Win,Lose",
    "key:GenericChoice.Defined=Player.Opponent"
)]
#[case::fallback(
    "GenericChoice | Choices$ Win,Lose | FallbackAbility$ Win",
    "key:GenericChoice.FallbackAbility"
)]
#[case::branch_without_true(
    "Branch | BranchConditionSVar$ N | FalseSubAbility$ Lose",
    "key:Branch.shape"
)]
#[case::set_state_unspecialize(
    "SetState | Defined$ Self | Mode$ Unspecialize",
    "key:SetState.Mode"
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
