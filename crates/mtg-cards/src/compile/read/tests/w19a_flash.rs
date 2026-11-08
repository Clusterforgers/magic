//! Goal: a card that has flash as long as a condition holds reads as a static ability of the card in a hand and is written back as the same text; another keyword, subject or missing condition stays with the table.
use super::*;
use crate::refs::ZoneKind;

#[rstest]
#[case::desert(
    "~ has flash as long as you control a Desert.",
    r#"{"Static":{"affected":{"Each":[{"In":"Hand"},"This"]},"mods":[{"AddKeyword":"Flash"}],"condition":{"Compare":{"left":{"Count":[{"Subtype":"Desert"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"from":"Hand"}}"#
)]
#[case::merfolk(
    "This spell has flash as long as you control a Merfolk.",
    r#"{"Static":{"affected":{"Each":[{"In":"Hand"},"This"]},"mods":[{"AddKeyword":"Flash"}],"condition":{"Compare":{"left":{"Count":[{"Subtype":"Merfolk"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"from":"Hand"}}"#
)]
fn w19a_a_conditional_flash_reads_and_is_written_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::another_keyword("~ has haste as long as you control a Desert.")]
#[case::no_condition("~ has flash.")]
#[case::another_subject("Creatures you control have flash as long as you control a Desert.")]
fn w19a_near_misses_stay_with_the_table(#[case] text: &str) {
    let read = clause(text, ClauseKind::Static, false);
    assert!(
        !matches!(
            read,
            Some(Ability::Static {
                from: ZoneKind::Hand,
                ..
            })
        ),
        "{text}: {read:?}"
    );
}
