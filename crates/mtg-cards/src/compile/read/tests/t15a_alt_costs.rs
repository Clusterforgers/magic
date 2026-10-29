//! Goal: "You may <part> and <part> rather than pay this spell's mana cost." reads as a cast for both parts in place of the mana cost and is written back as the same text; one part, mana, an X, another verb or a third part stays with the table.
use super::*;

#[rstest]
#[case::life_and_a_blue_card(
    "You may pay 1 life and exile a blue card from your hand rather than pay this spell's mana cost.",
    r#"[{"PayLife":1},{"ExileFromHand":{"filter":[{"Color":"Blue"}]}}]"#
)]
#[case::more_life_and_a_red_card(
    "You may pay 3 life and exile a red card from your hand rather than pay this spell's mana cost.",
    r#"[{"PayLife":3},{"ExileFromHand":{"filter":[{"Color":"Red"}]}}]"#
)]
#[case::a_discard_and_life(
    "You may discard a card and pay 2 life rather than pay this spell's mana cost.",
    r#"[{"Discard":{}},{"PayLife":2}]"#
)]
fn t15a_two_parts_read_and_are_written_back(#[case] text: &str, #[case] cost: &str) {
    let json = format!(r#"{{"FreeCast":{{"cost":{cost}}}}}"#);
    reads_as(text, ClauseKind::Static, false, &json);
}

#[rstest]
#[case::mana_is_no_part(
    "You may pay {1}{B} and exile a blue card from your hand rather than pay this spell's mana cost."
)]
#[case::an_x(
    "You may pay X life and exile a blue card from your hand rather than pay this spell's mana cost."
)]
#[case::another_verb(
    "You may pay 1 life and exile a blue card from your hand rather than cast this spell."
)]
#[case::no_second_part("You may pay 1 life and rather than pay this spell's mana cost.")]
#[case::a_third_part(
    "You may pay 1 life and exile a blue card from your hand and discard a card rather than pay this spell's mana cost."
)]
fn t15a_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
