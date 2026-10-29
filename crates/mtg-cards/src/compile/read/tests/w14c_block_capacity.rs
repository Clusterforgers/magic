//! Goal: "can block an additional creature each combat" reads as a static that lets the card or each creature of a group block one more attacker (two with "an additional two creatures"), is written back as the same text, and a near-miss (blocked rather than block, another span of time, no count) stays with the table.
use super::*;

#[rstest]
#[case::the_card_itself(
    "~ can block an additional creature each combat.",
    r#"{"Static":{"affected":"This","mods":[{"BlocksAdditional":1}]}}"#
)]
#[case::each_creature_you_control(
    "Each creature you control can block an additional creature each combat.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"BlocksAdditional":1}]}}"#
)]
fn w14c_a_creature_that_blocks_more_reads_and_is_written_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::blocked("~ can be blocked by an additional creature each combat.")]
#[case::each_turn("~ can block an additional creature each turn.")]
#[case::no_count("~ can block an additional each combat.")]
#[case::one_more_by_number("~ can block an additional 1 creatures each combat.")]
fn w14c_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
