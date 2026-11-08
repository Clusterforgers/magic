//! Goal: a die roll with one row of results is written as cards print it inline ("roll a six-sided die. You gain life equal to the result.", "If the result is 3 or less, ..."), and another die, another bound, a row that does not start at 1, a second row or another effect is not that text.
use super::*;

/// Whether one of the wordings the checker accepts for the ability is the text.
fn writes(json: &str, text: &str) -> bool {
    super::super::render::written_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

fn printed(json: &str) -> Vec<String> {
    super::super::render::written_variants(&ability(json))
}

/// An enters trigger that rolls a die of `sides` with the rows `rows`.
fn roll(sides: u32, rows: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"RollDice":{{"sides":{sides},"results":{rows}}}}}]}}}}"#
    )
}

const GAIN_ALL: &str = r#"[{"from":1,"effects":[{"GainLife":{"amount":{"DieResult":"Only"}}}]}]"#;
const LOSE_LOW: &str =
    r#"[{"from":1,"to":3,"effects":[{"LoseLife":{"amount":{"DieResult":"Only"}}}]}]"#;
const LOSE_HIGH: &str = r#"[{"from":4,"effects":[{"LoseLife":{"amount":{"DieResult":"Only"}}}]}]"#;
const TWO_ROWS: &str = r#"[{"from":1,"to":3,"effects":[{"LoseLife":{"amount":{"DieResult":"Only"}}}]},{"from":4,"effects":[{"Draw":{}}]}]"#;
const KITTEN: &str =
    "When this creature enters, roll a six-sided die. You gain life equal to the result.";
const CUSTOMER: &str = "When this creature enters, roll a six-sided die. If the result is 3 or less, you lose that much life.";

#[rstest]
#[case::gain_the_result(roll(6, GAIN_ALL), KITTEN, true)]
#[case::lose_at_three_or_less(roll(6, LOSE_LOW), CUSTOMER, true)]
#[case::a_twenty_sided_die(
    roll(20, GAIN_ALL),
    "When this creature enters, roll a d20. You gain life equal to the result.",
    true
)]
#[case::not_another_die(roll(4, GAIN_ALL), KITTEN, false)]
#[case::not_a_bound_row_as_every_result(
    roll(6, LOSE_LOW),
    "When this creature enters, roll a six-sided die. You lose life equal to the result.",
    false
)]
#[case::not_every_result_as_a_bound(
    roll(6, GAIN_ALL),
    "When this creature enters, roll a six-sided die. If the result is 3 or less, you gain that much life.",
    false
)]
#[case::not_another_bound(
    roll(6, LOSE_LOW),
    "When this creature enters, roll a six-sided die. If the result is 4 or less, you lose that much life.",
    false
)]
#[case::not_the_high_results(roll(6, LOSE_HIGH), CUSTOMER, false)]
#[case::not_two_rows(roll(6, TWO_ROWS), CUSTOMER, false)]
#[case::not_gain_for_lose(
    roll(6, LOSE_LOW),
    "When this creature enters, roll a six-sided die. If the result is 3 or less, you gain that much life.",
    false
)]
fn a_roll_with_one_row_is_written_inline(
    #[case] json: String,
    #[case] text: &str,
    #[case] expected: bool,
) {
    assert_eq!(writes(&json, text), expected, "{:?}", printed(&json));
}
