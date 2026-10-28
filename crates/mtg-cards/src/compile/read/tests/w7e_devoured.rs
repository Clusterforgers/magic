//! Goal: "for each creature it devoured" in an enters trigger reads into `Amount::Devoured`, alone and times a number, and writes back as the same text; another counted phrase stays something else.
use super::*;

#[rstest]
#[case::draw(
    "When this creature enters, draw a card for each creature it devoured.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{"count":"Devoured"}}]}}"#
)]
#[case::life_times_two(
    "When this creature enters, you gain 2 life for each creature it devoured.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"GainLife":{"amount":{"Times":{"by":2,"of":"Devoured"}}}}]}}"#
)]
fn the_creatures_it_devoured_read_and_write_back(#[case] text: &str, #[case] written: &str) {
    reads_as(text, ClauseKind::Triggered, false, written);
}

#[rstest]
#[case::another_permanent(
    "When this creature enters, draw a card for each creature they devoured."
)]
#[case::devoured_this_way(
    "When this creature enters, draw a card for each creature devoured this way."
)]
fn near_misses_are_not_devoured(#[case] text: &str) {
    let read = clause(text, ClauseKind::Triggered, false)
        .map(|ability| serde_json::to_string(&ability).unwrap_or_default());
    assert!(
        !read.is_some_and(|json| json.contains("Devoured")),
        "{text}"
    );
}
