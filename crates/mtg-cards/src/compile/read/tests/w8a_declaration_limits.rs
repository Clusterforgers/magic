//! Goal: a limit on the whole attack or block declaration reads into `AttackLimit` or `BlockLimit` with its count, writes back as the card's own text, and the near-misses ("this turn", a count that is not a word) stay with the table.
use super::*;

#[rstest]
#[case::one_attacks(
    "No more than one creature can attack each combat.",
    r#"{"Rule":{"rule":{"AttackLimit":1}}}"#
)]
#[case::one_blocks(
    "No more than one creature can block each combat.",
    r#"{"Rule":{"rule":{"BlockLimit":1}}}"#
)]
#[case::two_attack(
    "No more than two creatures can attack each combat.",
    r#"{"Rule":{"rule":{"AttackLimit":2}}}"#
)]
#[case::two_block(
    "No more than two creatures can block each combat.",
    r#"{"Rule":{"rule":{"BlockLimit":2}}}"#
)]
fn w8a_the_limits_read_into_the_dsl_and_write_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::each_turn("No more than one creature can attack each turn.")]
#[case::plural_of_one("No more than one creatures can attack each combat.")]
#[case::singular_of_two("No more than two creature can block each combat.")]
#[case::zero("No more than zero creatures can attack each combat.")]
#[case::other_verb("No more than one creature can untap each combat.")]
fn w8a_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
