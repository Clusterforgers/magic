//! Goal: "Whenever this Equipment becomes unattached from a permanent, sacrifice that permanent." reads as an
//! `Unattached` trigger whose sacrifice takes the permanent of the event, and is written back as the card words it; a
//! near miss of the event or of the permanent reads as something else or not at all.
use super::*;

const EXOSKELETON: &str = r#"{"Triggered":{"when":{"Unattached":{"what":["This"]}},"effects":[{"Sacrifice":{"filter":[{"Is":"Triggering"}]}}]}}"#;

/// Whether one of the wordings of the ability is the text.
fn writes(json: &str, text: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

#[test]
fn dk26_grafted_exoskeleton_reads_and_writes_back() {
    reads_as(
        "Whenever this Equipment becomes unattached from a permanent, sacrifice that permanent.",
        ClauseKind::Triggered,
        false,
        EXOSKELETON,
    );
}

#[rstest]
#[case::attached(
    "Whenever this Equipment becomes attached to a permanent, sacrifice that permanent."
)]
#[case::from_a_creature(
    "Whenever this Equipment becomes unattached from a creature, sacrifice that permanent."
)]
#[case::each_player(
    "Whenever this Equipment becomes unattached from a permanent, each player sacrifices that permanent."
)]
#[case::a_permanent(
    "Whenever this Equipment becomes unattached from a permanent, sacrifice a permanent."
)]
fn dk26_a_near_miss_is_not_the_exoskeleton(#[case] text: &str) {
    let read = clause(text, ClauseKind::Triggered, false);
    assert_ne!(read, Some(ability(EXOSKELETON)), "{text}");
    assert!(!writes(EXOSKELETON, text), "{text}");
}

#[rstest]
#[case::sacrifice_that_creature(
    "Whenever this Equipment becomes unattached from a permanent, sacrifice that creature."
)]
#[case::exile_that_permanent(
    "Whenever this Equipment becomes unattached from a permanent, sacrifice it."
)]
fn dk26_another_wording_of_the_permanent_is_not_written(#[case] text: &str) {
    assert!(!writes(EXOSKELETON, text), "{text}");
}
