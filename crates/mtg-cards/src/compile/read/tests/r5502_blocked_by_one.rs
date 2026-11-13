//! Goal: "must be blocked by a <kind> if able" of the card or of the equipped creature reads into `MustBeBlockedByOne`, writes back as the card's own text (alone and after the Equipment's bonus in one sentence), and the near-misses ("all Walls able to block", no "if able", another subject) do not read as it.
use super::*;

#[rstest]
#[case::the_card(
    "~ must be blocked by a Wall if able.",
    r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":{"MustBeBlockedByOne":[{"Subtype":"Wall"}]}}}}}"#
)]
#[case::equipped(
    "Equipped creature must be blocked by an Eldrazi if able.",
    r#"{"Rule":{"rule":{"Combat":{"who":"Attached","quirk":{"MustBeBlockedByOne":[{"Subtype":"Eldrazi"}]}}}}}"#
)]
fn r5502_the_requirement_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[test]
fn r5502_the_cleaver_sentence_is_written_from_its_two_abilities() {
    let bonus = ability(
        r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":3,"toughness":1}}]}}"#,
    );
    let rule = ability(
        r#"{"Rule":{"rule":{"Combat":{"who":"Attached","quirk":{"MustBeBlockedByOne":[{"Subtype":"Eldrazi"}]}}}}}"#,
    );
    let written = super::super::static_pairs::texts(&bonus, &rule);
    let wanted = "Equipped creature gets +3/+1 and must be blocked by an Eldrazi if able.";
    assert!(
        written.iter().any(|text| same_text(text, wanted)),
        "{written:?}"
    );
}

#[rstest]
#[case::every_wall("All Walls able to block ~ do so.")]
#[case::not_if_able("~ must be blocked by a Wall.")]
#[case::another_subject("Target creature must be blocked by a Wall if able.")]
fn r5502_near_misses_do_not_read_as_one_blocker_of_a_kind(#[case] text: &str) {
    let read = clause(text, ClauseKind::Static, false);
    let written = serde_json::to_string(&read).unwrap_or_default();
    assert!(!written.contains("MustBeBlockedByOne"), "{text}: {read:?}");
}
