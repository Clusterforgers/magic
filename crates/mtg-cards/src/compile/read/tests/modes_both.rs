//! Goal: "Choose one. If you control a commander as you cast this spell, you may choose both instead." reads as the `all_if` condition of a pair of modes, writes back in the spell and trigger wordings, and every near-miss stays with the table.
use super::*;
use crate::effect::Effect;

const JESKA: &str = "Choose one. If you control a commander as you cast this spell, you may choose both instead.\n\u{2022} Add {R} for each card in target opponent's hand.\n\u{2022} Exile the top three cards of your library. You may play them this turn.";
const CONDITION: &str =
    r#"{"Compare":{"left":{"Count":["Commander",{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#;

/// The condition of the one `Modal` the ability holds.
fn all_if(read: &Ability) -> Option<serde_json::Value> {
    let [Effect::Modal { all_if, modes, .. }] = read.effects() else {
        panic!("not one modal: {read:?}");
    };
    assert_eq!(modes.len(), 2);
    all_if
        .as_ref()
        .map(|condition| serde_json::to_value(condition).unwrap())
}

#[test]
fn a_spell_with_both_modes_for_a_commander_reads_and_writes_back() {
    let read = golden(JESKA, ClauseKind::Modal, false);
    let want: serde_json::Value = serde_json::from_str(CONDITION).unwrap();
    assert_eq!(all_if(&read), Some(want));
}

#[test]
fn a_trigger_words_the_condition_without_the_cast() {
    let text = "At the beginning of combat on your turn, choose one. If you control a commander, you may choose both instead.\n\u{2022} Create a 1/1 white Soldier creature token.\n\u{2022} You gain 2 life.";
    let read = golden(text, ClauseKind::Triggered, false);
    let want: serde_json::Value = serde_json::from_str(CONDITION).unwrap();
    assert_eq!(all_if(&read), Some(want));
}

#[test]
fn a_choice_without_the_sentence_has_no_condition() {
    let text = "Choose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.";
    let read = golden(text, ClauseKind::Modal, false);
    assert_eq!(all_if(&read), None);
}

#[rstest]
#[case::all_of_three(
    "Choose one. If you control a commander as you cast this spell, you may choose all instead.\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.\n\u{2022} Scry 1."
)]
#[case::three_modes(
    "Choose one. If you control a commander as you cast this spell, you may choose both instead.\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.\n\u{2022} Scry 1."
)]
#[case::choose_two(
    "Choose two. If you control a commander as you cast this spell, you may choose both instead.\n\u{2022} Draw a card.\n\u{2022} You gain 2 life."
)]
#[case::no_condition(
    "Choose one. You may choose both instead.\n\u{2022} Draw a card.\n\u{2022} You gain 2 life."
)]
#[case::an_unread_mode(
    "Choose one. If you control a commander as you cast this spell, you may choose both instead.\n\u{2022} Draw a card.\n\u{2022} Wish for a card."
)]
#[case::at_random(
    "Choose one at random. If you control a commander as you cast this spell, you may choose both instead.\n\u{2022} Draw a card.\n\u{2022} You gain 2 life."
)]
fn the_other_wordings_of_a_pair_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Modal, false), None, "{text}");
}
