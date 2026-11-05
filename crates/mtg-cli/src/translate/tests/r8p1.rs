//! Goal: round 8 piece 1 translates the prohibitions and counters it closes into the rules the engine reads, and keeps what it cannot say a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn artifact(line: &str) -> String {
    format!("Name:T\nTypes:Artifact\nS:{line}\n")
}

fn cant(who: &Value, action: &Value) -> Value {
    normal(json!({"Rule": {"rule": {"Cant": {"who": who, "action": action}}}}))
}

#[rstest]
#[case::null_rod(
    "Mode$ CantBeActivated | ValidCard$ Artifact | ValidSA$ Activated",
    cant(&json!("EachPlayer"), &json!({"ActivateKind": {"kind": "Every", "sources": [{"Type": "Artifact"}]}}))
)]
#[case::opponents_creatures(
    "Mode$ CantBeActivated | ValidCard$ Creature.OppCtrl | ValidSA$ Activated | AffectedZone$ Battlefield",
    cant(&json!("EachPlayer"), &json!({"ActivateKind": {"kind": "Every", "sources": [{"Type": "Creature"}, {"ControlledBy": "Opponents"}]}}))
)]
#[case::by_opponents(
    "Mode$ CantBeActivated | ValidCard$ Creature | ValidSA$ Activated | Activator$ Opponent",
    cant(&json!("Opponents"), &json!({"ActivateKind": {"kind": "Every", "sources": [{"Type": "Creature"}]}}))
)]
#[case::without_valid_sa(
    "Mode$ CantBeActivated | ValidCard$ Artifact | Activator$ You",
    cant(&json!("You"), &json!({"ActivateKind": {"kind": "Every", "sources": [{"Type": "Artifact"}]}}))
)]
#[case::no_mana_ability_stays_the_plain_form(
    "Mode$ CantBeActivated | ValidCard$ Artifact | ValidSA$ Activated.!ManaAbility",
    cant(&json!({"Each": [{"Type": "Artifact"}]}), &json!({"Activate": []}))
)]
fn activated_abilities_include_mana_abilities(#[case] line: &str, #[case] expected: Value) {
    assert_eq!(gaps(&artifact(line)), Vec::<String>::new());
    assert_eq!(abilities(&artifact(line)), vec![expected]);
}

#[rstest]
#[case::no_card(
    "Mode$ CantBeActivated | Activator$ You",
    "key:Static.CantBeActivated.no-ValidCard"
)]
#[case::named_card(
    "Mode$ CantBeActivated | ValidCard$ Card.NamedCard",
    "key:Static.CantBeActivated.ValidCard"
)]
fn what_the_prohibition_cannot_say_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert!(abilities(&artifact(line)).is_empty());
    assert!(!gaps(&artifact(line)).is_empty(), "{gap}");
}

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Artifact\nA:AB$ Investigate | {line} | SpellDescription$ x\n")
}

#[rstest]
#[case::once("", json!({"CreateToken": {"token": {"Named": "clue"}}}))]
#[case::four_times("Num$ 4", json!({"CreateToken": {"token": {"Named": "clue"}, "count": 4}}))]
#[case::the_defaults_are_left_out("Num$ 1 | Defined$ You", json!({"CreateToken": {"token": {"Named": "clue"}}}))]
fn investigate_reads_its_count_and_player(#[case] extra: &str, #[case] node: Value) {
    let line = if extra.is_empty() {
        "Cost$ T".to_owned()
    } else {
        format!("Cost$ T | {extra}")
    };
    let found = abilities(&spell(&line));
    assert!(gaps(&spell(&line)).is_empty(), "{found:?}");
    assert!(
        found[0]
            .to_string()
            .contains(&node.to_string()[1..node.to_string().len() - 1]),
        "{found:?}"
    );
}
