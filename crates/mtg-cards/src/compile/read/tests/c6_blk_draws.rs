//! Goal: "If you would draw a card, draw two cards instead." and its form "except the first N you draw in each of your draw steps" read as a `Draw` replacement with the number spared and write back as the card's text; other doublings and exceptions stay with the table.
use super::*;

#[rstest]
#[case::every_draw(
    "If you would draw a card, draw two cards instead.",
    r#"{"Replacement":{"when":{"Draw":{}},"by":{"Multiply":2}}}"#
)]
#[case::the_first_one(
    "If you would draw a card except the first one you draw in each of your draw steps, draw two cards instead.",
    r#"{"Replacement":{"when":{"Draw":{"except_draw_step":1}},"by":{"Multiply":2}}}"#
)]
#[case::the_first_two(
    "If you would draw a card except the first two you draw in each of your draw steps, draw two cards instead.",
    r#"{"Replacement":{"when":{"Draw":{"except_draw_step":2}},"by":{"Multiply":2}}}"#
)]
fn a_draw_is_doubled(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::three_cards("If you would draw a card, draw three cards instead.")]
#[case::each_turn(
    "If you would draw a card except the first one you draw each turn, draw two cards instead."
)]
#[case::an_opponent("If an opponent would draw a card, draw two cards instead.")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
