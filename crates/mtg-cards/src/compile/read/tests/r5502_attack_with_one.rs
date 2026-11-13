//! Goal: "each opponent must attack (you or a planeswalker you control) with at least one creature each combat if able" reads into `MustAttackWithOne`, writes back as the card's own text, and the near-misses (another player group, another count, another defender, "this turn") stay with the table.
use super::*;

#[rstest]
#[case::any_defender(
    "Each opponent must attack with at least one creature each combat if able.",
    r#"{"Rule":{"rule":{"MustAttackWithOne":{"who":"Opponents"}}}}"#
)]
#[case::you_or_your_planeswalker(
    "Each opponent must attack you or a planeswalker you control with at least one creature each combat if able.",
    r#"{"Rule":{"rule":{"MustAttackWithOne":{"who":"Opponents","at":{"Union":["You",{"Each":[{"Type":"Planeswalker"},{"ControlledBy":"You"}]}]}}}}}"#
)]
fn r5502_the_requirement_reads_into_the_dsl_and_writes_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::each_player("Each player must attack with at least one creature each combat if able.")]
#[case::two_creatures("Each opponent must attack with at least two creatures each combat if able.")]
#[case::only_you("Each opponent must attack you with at least one creature each combat if able.")]
#[case::this_turn("Each opponent must attack with at least one creature this turn if able.")]
#[case::not_if_able("Each opponent must attack with at least one creature each combat.")]
fn r5502_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
