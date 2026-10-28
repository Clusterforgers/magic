//! Goal: a clash reads into the DSL alone, with a win branch, or after steps it repeats on a win, and writes back as the card's own text; a lose branch, a repeat without steps and other players stay with the table.
use super::*;

#[rstest]
#[case::alone("Clash with an opponent.", r#"{"Spell":{"effects":[{"Clash":{}}]}}"#)]
#[case::a_win_branch(
    "Clash with an opponent. If you win, draw a card.",
    r#"{"Spell":{"effects":[{"Clash":{"win":[{"Draw":{}}]}}]}}"#
)]
#[case::lash_out(
    "~ deals 3 damage to target creature. Clash with an opponent. If you win, ~ deals 3 damage to that creature's controller.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}},{"Clash":{"win":[{"DealDamage":{"to":{"ControllerOf":{"Target":0}},"amount":3}}]}}]}}"#
)]
#[case::repeat_this_process(
    "You lose 2 life and draw two cards, then clash with an opponent. If you win, repeat this process.",
    r#"{"Spell":{"effects":[{"LoseLife":{"amount":2}},{"Draw":{"count":2}},{"Clash":{"win":[{"LoseLife":{"amount":2}},{"Draw":{"count":2}}],"until":true}}]}}"#
)]
fn w9b_clash_wordings_read_and_write_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::a_lose_branch("Clash with an opponent. If you lose, draw a card.")]
#[case::repeat_without_steps("Clash with an opponent. If you win, repeat this process.")]
#[case::another_player("Target player clashes with an opponent.")]
fn w9b_clash_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
