//! Goal: a counter moved from one target onto a second target reads as a move between the two targets and is written back as the same text; the same target twice or a player as the second stays with the table.
use super::*;

#[rstest]
#[case::plus_one("Move a +1/+1 counter from target creature onto a second target creature.")]
#[case::two_counters("Move two +1/+1 counters from target creature onto a second target creature.")]
fn w19b_a_counter_moved_between_two_targets_reads_and_is_written_back(#[case] text: &str) {
    let read = spell(text).unwrap_or_else(|| panic!("not read: {text}"));
    let wordings = render_variants(&read);
    assert!(
        wordings.iter().any(|written| same_text(written, text)),
        "{text}: {wordings:?}"
    );
}

#[rstest]
#[case::the_same_target("Move a +1/+1 counter from target creature onto target creature.")]
#[case::a_player("Move a +1/+1 counter from target creature onto a second target player.")]
#[case::a_second_without_a_first("Move a +1/+1 counter from ~ onto a second target creature.")]
fn w19b_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
