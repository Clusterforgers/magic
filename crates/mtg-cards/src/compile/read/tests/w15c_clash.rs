//! Goal: the clash wordings of `w15c_clash` (a clash with the defending player, a free cast from the hand with the limit after the hand) and "whenever you win a clash" read into the DSL and write back as the card's own text; near misses stay with the table.
use super::*;

#[rstest]
#[case::marvo_attacks(
    "Whenever ~ attacks, clash with defending player.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Clash":{"with":"Defending"}}]}}"#
)]
#[case::marvo_wins(
    "Whenever you win a clash, draw a card. Then you may cast a spell from your hand with mana value 8 or less without paying its mana cost.",
    r#"{"Triggered":{"when":{"W15cClashWon":{"who":"You"}},"effects":[{"Draw":{}},{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},{"Not":{"Type":"Land"}},{"Compare":{"stat":"ManaValue","op":"Le","to":8}}]},"count":{"min":0,"max":1},"as":"spell"}},{"CastFromZone":{"what":{"Bound":"spell"},"free":true}}]}}"#
)]
#[case::anyone(
    "Whenever a player wins a clash, draw a card.",
    r#"{"Triggered":{"when":{"W15cClashWon":{}},"effects":[{"Draw":{}}]}}"#
)]
#[case::an_opponent(
    "Whenever an opponent wins a clash, draw a card.",
    r#"{"Triggered":{"when":{"W15cClashWon":{"who":"Opponents"}},"effects":[{"Draw":{}}]}}"#
)]
fn w15c_clash_wordings_read_and_write_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::defending_with_a_branch("Clash with defending player. If you win, draw a card.")]
#[case::delayed_mana(
    "Counter target spell. Clash with an opponent. If you win, at the beginning of your next main phase, you may add an amount of {C} equal to that spell's mana value."
)]
#[case::no_limit_after_the_hand(
    "You may cast a spell from your hand with power 8 or less without paying its mana cost."
)]
fn w15c_clash_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
