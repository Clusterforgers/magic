//! Goal: "Cast this spell only during combat[ on an opponent's turn]." reads as a casting restriction on the spell and is written back as the same text; another moment stays with the table.
use super::*;

#[rstest]
#[case::combat(
    "Cast this spell only during combat.",
    r#"{"CastOnly":{"condition":"InCombat"}}"#
)]
#[case::combat_on_an_opponents_turn(
    "Cast this spell only during combat on an opponent's turn.",
    r#"{"CastOnly":{"condition":{"All":["InCombat",{"Not":"YourTurn"}]}}}"#
)]
fn w17c_a_casting_restriction_reads_and_is_written_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::another_moment("Cast this spell only during an opponent's end step.")]
#[case::another_verb("Activate this spell only during combat.")]
#[case::no_moment("Cast this spell only.")]
fn w17c_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}

#[test]
fn w17c_the_restriction_is_a_rule_of_the_spell() {
    assert_eq!(
        crate::compile::classify::classify("Cast this spell only during combat.", true),
        ClauseKind::Static
    );
}
