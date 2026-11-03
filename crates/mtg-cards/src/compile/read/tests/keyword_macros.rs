//! Goal: keyword actions the macro table expands read from every wording and number into the table's nodes, and write back as the same text.
use super::*;

#[rstest]
#[case::earthbend_in_a_spell("Earthbend 2.", ClauseKind::Spell, r#""count":2"#)]
#[case::earthbend_in_a_trigger("When ~ enters, earthbend 1.", ClauseKind::Triggered, r#""Haste""#)]
#[case::earthbend_with_a_big_number("Earthbend 5.", ClauseKind::Spell, r#""count":5"#)]
#[case::endure_by_name("When ~ enters, ~ endures 2.", ClauseKind::Triggered, r#""Spirit""#)]
fn a_keyword_action_reads_into_the_nodes_of_its_entry(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] part: &str,
) {
    let read = golden(text, kind, false);
    let json = serde_json::to_string(&read).unwrap_or_default();
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::no_number("Earthbend.")]
#[case::zero("Earthbend 0.")]
#[case::endure_without_number("When ~ enters, ~ endures.")]
fn a_keyword_action_without_a_number_stays_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Spell, false), None, "{text}");
}

#[rstest]
#[case::target_in_a_spell("Target creature endures 2.", ClauseKind::Spell, r#""Target":0"#)]
#[case::target_in_a_trigger(
    "When ~ enters, target creature endures 3.",
    ClauseKind::Triggered,
    r#""count":3"#
)]
#[case::it_after_an_event("When ~ enters, it endures 1.", ClauseKind::Triggered, r#""size":1"#)]
fn endure_reads_of_another_creature_and_writes_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] part: &str,
) {
    let read = golden(text, kind, false);
    let json = serde_json::to_string(&read).unwrap_or_default();
    assert!(json.contains(part) && json.contains("Spirit"), "{json}");
}
