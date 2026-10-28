//! Goal: "if it had counters on it" in a trigger on a permanent that leaves reads as a count of the counters of any kind (or of one kind) on the object of the event, at least one, in each wording, and writes back as the card's text; a count other than "one or more" stays unread.
use super::*;

#[rstest]
#[case::angelic_sleuth(
    "Whenever another permanent you control leaves the battlefield, if it had counters on it, investigate.",
    r#"{"AnyCounters":"Triggering"}"#
)]
#[case::nikara(
    "Whenever another creature you control leaves the battlefield, if it had one or more counters on it, you draw a card and you lose 1 life.",
    r#"{"AnyCounters":"Triggering"}"#
)]
#[case::one_kind(
    "Whenever another creature you control dies, if it had one or more +1/+1 counters on it, draw a card.",
    r#"{"Counters":{"on":"Triggering","counter":"+1/+1"}}"#
)]
fn c11c_had_counters_reads_and_writes_back(#[case] text: &str, #[case] part: &str) {
    let json = serde_json::to_string(&golden(text, ClauseKind::Triggered, false)).unwrap();
    assert!(json.contains(part), "{text}: {json}");
}

#[rstest]
#[case::two_or_more(
    "Whenever another creature you control dies, if it had two or more counters on it, draw a card."
)]
#[case::has_now(
    "Whenever another creature you control dies, if it has counters on it, draw a card."
)]
fn c11c_had_counters_near_misses_stay_unread(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Triggered, false).is_none(),
        "{text}"
    );
}
