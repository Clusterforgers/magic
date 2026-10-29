//! Goal: a limit on the attacks at you or at this planeswalker reads into `AttackLimitAt` with its count and protected
//! defender, writes back as the card's own text, and the near-misses (another defender, "each turn", a count that does
//! not agree with its noun) stay with the table.
use super::*;

#[rstest]
#[case::one_at_you(
    "No more than one creature can attack you each combat.",
    r#"{"Rule":{"rule":{"AttackLimitAt":{"most":1,"at":"You"}}}}"#
)]
#[case::two_at_you(
    "No more than two creatures can attack you each combat.",
    r#"{"Rule":{"rule":{"AttackLimitAt":{"most":2,"at":"You"}}}}"#
)]
#[case::one_at_this(
    "No more than one creature can attack ~ each combat.",
    r#"{"Rule":{"rule":{"AttackLimitAt":{"most":1,"at":"This"}}}}"#
)]
fn w10a_the_limits_at_a_defender_read_and_write_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::an_opponent("No more than one creature can attack each opponent each combat.")]
#[case::each_turn("No more than one creature can attack you each turn.")]
#[case::plural_of_one("No more than one creatures can attack you each combat.")]
#[case::zero("No more than zero creatures can attack you each combat.")]
fn w10a_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
