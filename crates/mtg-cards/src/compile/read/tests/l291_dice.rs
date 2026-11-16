//! Goal: "Whenever you roll a 1", its lists ("a 1 or 2", "a 2, 4, or 6") and its thresholds ("a 4 or higher", "a 5 or higher on a die") read into `DiceRolled` with `results` or `at_least` and write back as the card's text; a list out of order, a repeated number, a list without "or", a zero, a natural threshold and "or lower" stay unread.
use super::*;

#[rstest]
#[case::one_number(
    "Whenever you roll a 1, create a 1/1 white Clown Robot artifact creature token.",
    r#"{"Triggered":{"when":{"DiceRolled":{"who":"You","results":[1]}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Clown Robot","colors":["White"],"types":["Artifact","Creature"],"subtypes":["Clown","Robot"],"power":1,"toughness":1}}}}]}}"#
)]
#[case::two_numbers(
    "Whenever you roll a 1 or 2, put that many +1/+1 counters on ~.",
    r#"{"Triggered":{"when":{"DiceRolled":{"who":"You","results":[1,2]}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1","count":"Triggering"}}]}}"#
)]
#[case::three_numbers(
    "Whenever you roll a 2, 4, or 6, draw a card.",
    r#"{"Triggered":{"when":{"DiceRolled":{"who":"You","results":[2,4,6]}},"effects":[{"Draw":{}}]}}"#
)]
#[case::an_opponent(
    "Whenever an opponent rolls a 6, you gain 1 life.",
    r#"{"Triggered":{"when":{"DiceRolled":{"who":"Opponents","results":[6]}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::four_or_higher_on_a_die(
    "Whenever you roll a 4 or higher on a die, create a 1/1 green Squirrel creature token.",
    r#"{"Triggered":{"when":{"DiceRolled":{"who":"You","at_least":4,"on_a_die":true}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Squirrel","colors":["Green"],"types":["Creature"],"subtypes":["Squirrel"],"power":1,"toughness":1}}}}]}}"#
)]
#[case::five_or_higher_on_a_die(
    "Whenever you roll a 5 or higher on a die, put a +1/+1 counter on ~.",
    r#"{"Triggered":{"when":{"DiceRolled":{"who":"You","at_least":5,"on_a_die":true}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#
)]
#[case::three_or_higher(
    "Whenever you roll a 3 or higher, you gain 1 life.",
    r#"{"Triggered":{"when":{"DiceRolled":{"who":"You","at_least":3}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::an_opponent_rolls_high(
    "Whenever an opponent rolls a 6 or higher, you gain 1 life.",
    r#"{"Triggered":{"when":{"DiceRolled":{"who":"Opponents","at_least":6}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
fn l291_dice_results_read_and_write_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::out_of_order("Whenever you roll a 2 or 1, draw a card.")]
#[case::repeated("Whenever you roll a 2 or 2, draw a card.")]
#[case::no_or("Whenever you roll a 1, 2, 3, draw a card.")]
#[case::comma_or_of_two("Whenever you roll a 1, or 2, draw a card.")]
#[case::zero("Whenever you roll a 0, draw a card.")]
#[case::zero_or_higher("Whenever you roll a 0 or higher, draw a card.")]
#[case::natural_or_higher("Whenever you roll a natural 19 or higher, draw a card.")]
#[case::or_lower("Whenever you roll a 3 or lower, draw a card.")]
fn l291_dice_results_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
