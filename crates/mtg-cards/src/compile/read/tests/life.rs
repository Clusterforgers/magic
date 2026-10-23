//! Goal: an intervening "if you have N or more life" and "you win the game" read in a trigger and a spell, and every other condition on a life total, other way to win and other player stays with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[test]
fn felidar_sovereign_reads_in_the_shape_of_its_file() {
    reads_as(
        "At the beginning of your upkeep, if you have 40 or more life, you win the game.",
        ClauseKind::Triggered,
        false,
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":40}},"effects":[{"WinGame":{}}]}}"#,
    );
}

#[rstest]
#[case::or_less(
    "At the beginning of your end step, if you have 5 or less life, you gain 5 life.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"if":{"Compare":{"left":{"Life":"You"},"op":"Le","right":5}},"effects":[{"GainLife":{"amount":5}}]}}"#
)]
#[case::a_spell_effect_after_the_condition(
    "At the beginning of each upkeep, if you have 20 or more life, draw a card. You gain 1 life.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep"}},"if":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}},"effects":[{"Draw":{}},{"GainLife":{"amount":1}}]}}"#
)]
fn an_intervening_if_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case("At the beginning of your upkeep, if you have 50 or more life, you win the game.")]
#[case("At the beginning of your end step, if you have 30 or more life, you win the game.")]
#[case("At the beginning of your upkeep, if you have 1 or less life, you gain 10 life.")]
#[case("Whenever you gain life, if you have 40 or more life, you win the game.")]
#[case("Whenever ~ attacks, if you have 10 or less life, draw a card.")]
#[case("When ~ enters, you win the game.")]
#[case("Whenever you cast a spell, you win the game.")]
fn more_oracle_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case("You win the game.")]
#[case("Target player wins the game.")]
#[case("Draw a card. You win the game.")]
fn winning_reads_in_a_spell(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case::more_than(
    "At the beginning of your upkeep, if you have more than 40 life, you win the game."
)]
#[case::the_life_first(
    "At the beginning of your upkeep, if you have 40 life or more, you win the game."
)]
#[case::exactly("At the beginning of your upkeep, if you have 40 life, you win the game.")]
#[case::an_opponent(
    "At the beginning of your upkeep, if an opponent has 0 or less life, you win the game."
)]
#[case::cards(
    "At the beginning of your upkeep, if you have 7 or more cards in hand, you win the game."
)]
#[case::a_creature(
    "At the beginning of your upkeep, if an opponent controls more lands than you, you win the game."
)]
#[case::no_comma("At the beginning of your upkeep, if you have 40 or more life you win the game.")]
#[case::unless(
    "At the beginning of your upkeep, you win the game unless you have 10 or less life."
)]
#[case::a_variable(
    "At the beginning of your upkeep, if you have X or more life, you win the game."
)]
#[case::losing("At the beginning of your upkeep, you lose the game.")]
#[case::an_opponent_loses("At the beginning of your upkeep, each opponent loses the game.")]
#[case::wins_with_a_count("At the beginning of your upkeep, you win the game twice.")]
#[case::win_a_coin_flip("At the beginning of your upkeep, you win the flip.")]
#[case::if_after_the_effect(
    "At the beginning of your upkeep, you win the game if you have 40 or more life."
)]
fn near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}
