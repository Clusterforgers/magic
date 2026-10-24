//! Goal: a loyalty ability reads from its cost (`+N`, `−N`, `0`, `−X`), a colon and the sentences of a spell, in the shapes of the hand-written planeswalkers, and writes back as the same text; a cost that is no loyalty cost, an `X` the cost does not name, a sentence the templates do not model and an "Activate only" restriction stay with the table.
use super::*;

fn loyalty(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Loyalty, false)
}

#[rstest]
#[case::plus(
    "+1: Draw a card.",
    r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"Draw":{}}]}}"#
)]
#[case::zero(
    "0: Draw a card.",
    r#"{"Loyalty":{"cost":"Zero","effects":[{"Draw":{}}]}}"#
)]
#[case::minus_damage_to_a_creature(
    "\u{2212}3: ~ deals 4 damage to target creature.",
    r#"{"Loyalty":{"cost":{"Minus":3},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":4}}]}}"#
)]
#[case::minus_two_sentences(
    "\u{2212}2: Draw a card. You gain 2 life.",
    r#"{"Loyalty":{"cost":{"Minus":2},"effects":[{"Draw":{}},{"GainLife":{"amount":2}}]}}"#
)]
#[case::minus_a_big_cost(
    "\u{2212}12: You gain 12 life.",
    r#"{"Loyalty":{"cost":{"Minus":12},"effects":[{"GainLife":{"amount":12}}]}}"#
)]
#[case::minus_x(
    "\u{2212}X: ~ deals X damage to target creature.",
    r#"{"Loyalty":{"cost":"MinusX","targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"X"}}]}}"#
)]
fn a_loyalty_ability_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Loyalty, false, json);
}

#[rstest]
#[case::no_colon("+1 Draw a card.")]
#[case::no_sign_and_a_number("1: Draw a card.")]
#[case::plus_x("+X: Draw a card.")]
#[case::minus_without_a_number("\u{2212}: Draw a card.")]
#[case::ascii_minus("-1: Draw a card.")]
#[case::no_effect("+1:")]
#[case::an_unread_sentence("+1: Wibble the wobble.")]
#[case::one_sentence_unread("+1: Draw a card. Wibble the wobble.")]
#[case::x_that_the_cost_does_not_name("+1: ~ deals X damage to target creature.")]
#[case::a_restriction("+1: Draw a card. Activate only as a sorcery.")]
#[case::an_activated_cost("{T}: Draw a card.")]
fn a_near_miss_of_a_loyalty_ability_stays_with_the_table(#[case] text: &str) {
    assert_eq!(loyalty(text), None, "{text}");
}

#[test]
fn the_text_of_a_spell_is_no_loyalty_ability() {
    assert_eq!(loyalty("Draw a card."), None);
    assert_eq!(loyalty("When ~ enters, draw a card."), None);
    assert_eq!(
        clause("+1: Draw a card.", ClauseKind::Spell, false),
        None,
        "a spell has no loyalty cost"
    );
}
