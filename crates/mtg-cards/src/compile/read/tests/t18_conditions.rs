//! Goal: an intervening "if you control another X" (a count that leaves the card itself out) writes with "another" for one kind, a list of kinds and a card type; a count that keeps the card, or a different kind, does not write it.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn enters_if(count: &str, op: &str, number: u32) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"Draw":{{}}}}],"if":{{"Compare":{{"left":{{"Count":[{count}]}},"op":"{op}","right":{number}}}}},"when":{{"ZoneChange":{{"to":"Battlefield","what":["This"]}}}}}}}}"#
    )
}

#[rstest]
#[case::subtype(
    r#"{"Subtype":"Human"},"Other",{"ControlledBy":"You"}"#,
    "When ~ enters, if you control another Human, draw a card."
)]
#[case::other_last(
    r#"{"Subtype":"Human"},{"ControlledBy":"You"},"Other""#,
    "When ~ enters, if you control another Human, draw a card."
)]
#[case::card_type(
    r#"{"Type":"Creature"},"Other",{"ControlledBy":"You"}"#,
    "When ~ enters, if you control another creature, draw a card."
)]
#[case::two_kinds(
    r#"{"AnyOf":[{"Subtype":"Human"},{"Subtype":"Elf"}]},"Other",{"ControlledBy":"You"}"#,
    "When ~ enters, if you control another Human or Elf, draw a card."
)]
fn control_another(#[case] count: &str, #[case] text: &str) {
    assert!(written(&enters_if(count, "Ge", 1), text));
}

#[rstest]
#[case::the_card_counts(
    r#"{"Subtype":"Human"},{"ControlledBy":"You"}"#,
    "Ge",
    1,
    "When ~ enters, if you control another Human, draw a card."
)]
#[case::an_opponent_controls(
    r#"{"Subtype":"Human"},"Other",{"ControlledBy":"Opponents"}"#,
    "Ge",
    1,
    "When ~ enters, if you control another Human, draw a card."
)]
#[case::another_kind(
    r#"{"Subtype":"Elf"},"Other",{"ControlledBy":"You"}"#,
    "Ge",
    1,
    "When ~ enters, if you control another Human, draw a card."
)]
#[case::no_other_leaves_none(
    r#"{"Subtype":"Human"},"Other",{"ControlledBy":"You"}"#,
    "Eq",
    0,
    "When ~ enters, if you control another Human, draw a card."
)]
fn not_the_same_rule(#[case] count: &str, #[case] op: &str, #[case] number: u32, #[case] text: &str) {
    assert!(!written(&enters_if(count, op, number), text));
}
