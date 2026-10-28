//! Goal: "You and target opponent each draw N cards." and its forms with another player read as your `Draw` and the other player's of the same count, also as a chapter, and write back as the card's text; another verb, a different count or a second "you" stays with the table.
use super::*;

#[rstest]
#[case::target_opponent(
    "You and target opponent each draw two cards.",
    r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"Draw":{"count":2}},{"Draw":{"who":{"Target":0},"count":2}}]}}"#
)]
#[case::target_player(
    "You and target player each draw a card.",
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Draw":{}},{"Draw":{"who":{"Target":0}}}]}}"#
)]
#[case::each_opponent(
    "You and each opponent each draw three cards.",
    r#"{"Spell":{"effects":[{"Draw":{"count":3}},{"Draw":{"who":"Opponents","count":3}}]}}"#
)]
fn you_and_another_player_each_draw(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[test]
fn a_chapter_reads_it() {
    let text = "I \u{2014} You and target opponent each draw two cards.";
    let read = crate::compile::read::chapter_block(text, false, &Own::default());
    let expected = ability(
        r#"{"Triggered":{"when":{"Chapter":{"number":1}},"targets":[{"what":"Opponent"}],"effects":[{"Draw":{"count":2}},{"Draw":{"who":{"Target":0},"count":2}}]}}"#,
    );
    assert_eq!(read, Some(vec![expected]));
}

#[rstest]
#[case::another_verb("You and target opponent each mill two cards.")]
#[case::you_twice("You and you each draw two cards.")]
#[case::no_each("You and target opponent draw two cards.")]
#[case::a_creature("You and target creature each draw a card.")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Spell, false), None, "{text}");
}
