//! Goal: a static ability that gives a keyword to the cards of a hand or a graveyard ("Each creature card in your hand has cycling {1}{U}.", "Nonland cards in your hand have miracle {0}.") reads into a `Static` over the cards of that zone, writes back as the same text, and leaves every near-miss to the table.
use super::*;

fn given(cards: &str, keyword: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":{{"Each":[{cards}]}},"mods":[{{"AddKeyword":{keyword}}}]}}}}"#
    )
}

#[rstest]
#[case::creature_cards_in_a_hand_cycle(
    "Each creature card in your hand has cycling {1}{U}.",
    r#"{"Type":"Creature"},{"In":"Hand"},{"OwnedBy":"You"}"#,
    r#"{"Cycling":"{1}{U}"}"#
)]
#[case::land_cards_in_a_hand_cycle(
    "Each land card in your hand has cycling {R}.",
    r#"{"Type":"Land"},{"In":"Hand"},{"OwnedBy":"You"}"#,
    r#"{"Cycling":"{R}"}"#
)]
#[case::nonland_cards_have_miracle(
    "Nonland cards in your hand have miracle {0}.",
    r#"{"Not":{"Type":"Land"}},{"In":"Hand"},{"OwnedBy":"You"}"#,
    r#"{"Miracle":"{0}"}"#
)]
#[case::cards_of_a_graveyard_retrace(
    "Each creature card in your graveyard has retrace.",
    r#"{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}"#,
    r#""Retrace""#
)]
fn cards_are_given_a_keyword(#[case] text: &str, #[case] cards: &str, #[case] keyword: &str) {
    reads_as(text, ClauseKind::Static, false, &given(cards, keyword));
}

#[rstest]
#[case::a_battlefield_group("Each creature you control has cycling {1}{U}.")]
#[case::a_keyword_a_card_does_not_use("Each creature card in your hand has flying.")]
#[case::a_cost_that_is_not_mana("Each creature card in your hand has cycling.")]
#[case::a_pump("Each creature card in your hand gets +1/+1.")]
#[case::a_condition_after(
    "Each creature card in your hand has cycling {1}{U} as long as you control a creature."
)]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
