//! Goal: a counter of an ability on the stack reads in a spell, a triggered and an activated ability, with each of its four target phrases, writes back as the same text, and a near-miss stays with the table.
use super::super::*;
use super::{golden, reads_as};
use rstest::rstest;

const C8_KWQ_ANYTHING: &str =
    r#"{"AnyOf":["Spell",{"Ability":"Activated"},{"Ability":"Triggered"}]}"#;
const C8_KWQ_EITHER: &str = r#"{"AnyOf":[{"Ability":"Activated"},{"Ability":"Triggered"}]}"#;

#[rstest]
#[case::anything("spell, activated ability, or triggered ability", C8_KWQ_ANYTHING)]
#[case::either("activated or triggered ability", C8_KWQ_EITHER)]
#[case::activated("activated ability", r#"{"Ability":"Activated"}"#)]
#[case::triggered("triggered ability", r#"{"Ability":"Triggered"}"#)]
fn c8_kwq_an_ability_is_countered_in_each_kind_of_ability(
    #[case] phrase: &str,
    #[case] selector: &str,
    #[values("spell", "exploit", "activated")] host: &str,
) {
    let target = format!(r#""targets":[{{"what":{{"Object":[{selector}]}}}}]"#);
    let counter = r#""effects":[{"Counter":{"what":{"Target":0}}}]"#;
    let (text, kind, json) = match host {
        "spell" => (
            format!("Counter target {phrase}."),
            ClauseKind::Spell,
            format!(r#"{{"Spell":{{{target},{counter}}}}}"#),
        ),
        "exploit" => (
            format!("When ~ exploits a creature, counter target {phrase}."),
            ClauseKind::Triggered,
            format!(
                r#"{{"Triggered":{{"when":{{"Exploits":{{"what":["This"]}}}},{target},{counter}}}}}"#
            ),
        ),
        _ => (
            format!("{{2}}{{U}}: Counter target {phrase}."),
            ClauseKind::Activated,
            format!(r#"{{"Activated":{{"cost":[{{"Mana":"{{2}}{{U}}"}}],{target},{counter}}}}}"#),
        ),
    };
    reads_as(&text, kind, false, &json);
}

#[rstest]
#[case::no_comma("Counter target spell activated ability or triggered ability.")]
#[case::of_a_creature("Counter target activated ability of a creature.")]
#[case::you_control("Counter target triggered ability you control.")]
#[case::two_kinds("Counter target spell or activated ability.")]
#[case::destroy("Destroy target activated ability.")]
fn c8_kwq_a_near_miss_stays_unread(#[case] text: &str) {
    assert!(clause(text, ClauseKind::Spell, false).is_none(), "{text}");
}

#[test]
fn c8_kwq_the_counter_writes_its_first_wording() {
    golden(
        "Counter target spell, activated ability, or triggered ability.",
        ClauseKind::Spell,
        false,
    );
}
