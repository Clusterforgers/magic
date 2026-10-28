//! Goal: a counter removed from a permanent the payer controls reads as a cost part with `from` and writes back as the card's text, for a named kind and each noun; a count other than one, "from among", a counter of any kind and a permanent of another player stay unread.
use super::*;

#[rstest]
#[case::ghave(
    "{1}, Remove a +1/+1 counter from a creature you control: Create a 1/1 green Saproling creature token.",
    r#"{"RemoveCounters":{"counter":"+1/+1","from":[{"Type":"Creature"}]}}"#
)]
#[case::quillspike(
    "{B/G}, Remove a -1/-1 counter from a creature you control: This creature gets +3/+3 until end of turn.",
    r#"{"RemoveCounters":{"counter":"-1/-1","from":[{"Type":"Creature"}]}}"#
)]
#[case::a_quest_counter(
    "{1}, Remove a quest counter from a permanent you control: Draw a card.",
    r#"{"RemoveCounters":{"counter":{"Other":"quest"},"from":"#
)]
#[case::another_creature(
    "{1}, Remove a +1/+1 counter from another creature you control: Draw a card.",
    r#"{"RemoveCounters":{"counter":"+1/+1","from":[{"Type":"Creature"},"Other"]}}"#
)]
#[case::the_source_still(
    "{T}, Remove a charge counter from ~: Draw a card.",
    r#"{"RemoveCounters":{"counter":"charge"}}"#
)]
fn c11c_counter_costs_read_and_write_back(#[case] text: &str, #[case] part: &str) {
    let json = serde_json::to_string(&golden(text, ClauseKind::Activated, false)).unwrap();
    assert!(json.contains(part), "{text}: {json}");
}

#[rstest]
#[case::two_counters("{1}, Remove two +1/+1 counters from a creature you control: Draw a card.")]
#[case::from_among("{1}, Remove two +1/+1 counters from among creatures you control: Draw a card.")]
#[case::any_kind("{T}, Remove a counter from a creature you control: Draw a card.")]
#[case::an_opponents(
    "{1}, Remove a +1/+1 counter from a creature an opponent controls: Draw a card."
)]
#[case::no_controller("{1}, Remove a +1/+1 counter from a creature: Draw a card.")]
fn c11c_counter_cost_near_misses_stay_unread(#[case] text: &str) {
    let json = clause(text, ClauseKind::Activated, false)
        .map(|read| serde_json::to_string(&read).unwrap());
    assert!(
        !json
            .as_deref()
            .is_some_and(|json| json.contains(r#""from":"#)),
        "{text}: {json:?}"
    );
}
