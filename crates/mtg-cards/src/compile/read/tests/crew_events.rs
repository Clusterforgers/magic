//! Goal: the triggers "becomes crewed" and "becomes saddled" of a Vehicle or Mount, also "for the first time each turn", read into the patterns the engine runs and write back as the card's own text, and the wordings around them that no pattern holds stay unread.
use super::*;

#[rstest]
#[case::crewed(
    "Whenever ~ becomes crewed, you gain 1 life.",
    r#"{"Triggered":{"when":{"BecomesCrewed":{"what":["This"]}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::saddled(
    "Whenever ~ becomes saddled, you gain 1 life.",
    r#"{"Triggered":{"when":{"BecomesSaddled":{"what":["This"]}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::crewed_the_first_time(
    "Whenever ~ becomes crewed for the first time each turn, draw a card.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"BecomesCrewed":{"what":["This"]}},"nth":1}},"effects":[{"Draw":{}}]}}"#
)]
#[case::saddled_the_first_time(
    "Whenever ~ becomes saddled for the first time each turn, draw a card.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"BecomesSaddled":{"what":["This"]}},"nth":1}},"effects":[{"Draw":{}}]}}"#
)]
#[case::crewed_the_second_time(
    "Whenever ~ becomes crewed for the second time each turn, draw a card.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"BecomesCrewed":{"what":["This"]}},"nth":2}},"effects":[{"Draw":{}}]}}"#
)]
fn becoming_crewed_or_saddled_reads_and_writes_back(#[case] text: &str, #[case] written: &str) {
    reads_as(text, ClauseKind::Triggered, false, written);
}

#[rstest]
#[case::a_vehicle_you_control("Whenever a Vehicle you control becomes crewed, draw a card.")]
#[case::a_mount_you_control("Whenever a Mount you control becomes saddled, draw a card.")]
#[case::crewed_or_dies("Whenever ~ becomes crewed or dies, draw a card.")]
fn the_permanents_that_become_crewed_or_saddled_and_a_list_with_another_event_read(
    #[case] text: &str,
) {
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::crewed_or_saddled("Whenever ~ becomes crewed or saddled, draw a card.")]
#[case::a_count_in_a_list(
    "Whenever ~ becomes crewed for the first time each turn or dies, draw a card."
)]
#[case::crewed_by_something("Whenever ~ becomes crewed by a Pilot, draw a card.")]
#[case::the_zeroth_time("Whenever ~ becomes crewed for the zeroth time each turn, draw a card.")]
#[case::once_per_game("Whenever ~ becomes crewed for the first time, draw a card.")]
#[case::a_batch("Whenever one or more Vehicles you control become crewed, draw a card.")]
fn other_wordings_of_the_crew_events_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
