//! Goal: "While voting, you may vote an additional time." reads as the rule that its controller may vote an additional time and is written back as the same text; another player or count stays with the table.
use super::*;

#[test]
fn w17c_the_additional_vote_reads_and_is_written_back() {
    reads_as(
        "While voting, you may vote an additional time.",
        ClauseKind::Static,
        false,
        r#"{"Rule":{"rule":{"VoteAgain":"You"}}}"#,
    );
}

#[rstest]
#[case::another_player("While voting, each opponent may vote an additional time.")]
#[case::another_count("While voting, you may vote two additional times.")]
fn w17c_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
