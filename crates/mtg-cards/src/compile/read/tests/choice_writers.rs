//! Goal: the choice of modes is written as the card words it (how many are chosen, a mode's name before its text or not, "that creature" for the creature of the event), and another number, name or mode is not its text.
use super::*;

/// Whether one of the wordings of the ability is the text.
fn writes(json: &str, text: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

const DRAW: &str = r#"{"effects":[{"Draw":{}}]}"#;
const DRAW_NAMED: &str = r#"{"label":"Sort Inventory","effects":[{"Draw":{}}]}"#;
const GAIN: &str = r#"{"effects":[{"GainLife":{"amount":2}}]}"#;
const COUNTER_ON_IT: &str =
    r#"{"label":"Pick a Perk","effects":[{"PutCounters":{"on":"Triggering","counter":"+1/+1"}}]}"#;

/// A trigger on the card entering whose effect is a choice.
fn entering(choose: &str, modes: &[&str]) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"Modal":{{{choose}"modes":[{}]}}}}]}}}}"#,
        modes.join(",")
    )
}

#[rstest]
#[case::one("", &[DRAW, GAIN], "When ~ enters, choose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.")]
#[case::one_or_both(r#""choose":{"min":1,"max":2},"#, &[DRAW, GAIN], "When ~ enters, choose one or both \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.")]
#[case::two_of_three(r#""choose":{"min":2,"max":2},"#, &[DRAW, GAIN, DRAW], "When ~ enters, choose two \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.\n\u{2022} Draw a card.")]
#[case::up_to_one(r#""choose":{"min":0,"max":1},"#, &[DRAW, GAIN], "When ~ enters, choose up to one \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.")]
#[case::one_or_more(r#""choose":{"min":1,"max":null},"#, &[DRAW, GAIN], "When ~ enters, choose one or more \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.")]
#[case::pip_boy_names_the_modes(
    "",
    &[DRAW_NAMED, COUNTER_ON_IT],
    "When ~ enters, choose one \u{2014}\n\u{2022} Sort Inventory \u{2014} Draw a card.\n\u{2022} Pick a Perk \u{2014} Put a +1/+1 counter on that creature."
)]
#[case::a_name_the_card_does_not_print(
    "",
    &[DRAW_NAMED, GAIN],
    "When ~ enters, choose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life."
)]
fn a_choice_is_written_as_the_card_words_it(
    #[case] choose: &str,
    #[case] modes: &[&str],
    #[case] text: &str,
) {
    let json = entering(choose, modes);
    assert!(
        writes(&json, text),
        "{text}: {:?}",
        render_variants(&ability(&json))
    );
}

#[rstest]
#[case::two_for_one(
    "",
    &[DRAW, GAIN],
    "When ~ enters, choose two \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life."
)]
#[case::both_for_one(
    "",
    &[DRAW, GAIN],
    "When ~ enters, choose one or both \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life."
)]
#[case::the_modes_in_another_order(
    "",
    &[DRAW, GAIN],
    "When ~ enters, choose one \u{2014}\n\u{2022} You gain 2 life.\n\u{2022} Draw a card."
)]
#[case::another_name(
    "",
    &[DRAW_NAMED, GAIN],
    "When ~ enters, choose one \u{2014}\n\u{2022} Check Map \u{2014} Draw a card.\n\u{2022} You gain 2 life."
)]
#[case::a_name_the_data_lacks(
    "",
    &[DRAW, GAIN],
    "When ~ enters, choose one \u{2014}\n\u{2022} Sort Inventory \u{2014} Draw a card.\n\u{2022} You gain 2 life."
)]
fn another_choice_is_not_the_text(
    #[case] choose: &str,
    #[case] modes: &[&str],
    #[case] text: &str,
) {
    let json = entering(choose, modes);
    assert!(!writes(&json, text), "{text}");
}

#[rstest]
#[case::both_of_three(r#""choose":{"min":1,"max":2},"#, &[DRAW, GAIN, DRAW])]
#[case::three_of_two(r#""choose":{"min":3,"max":3},"#, &[DRAW, GAIN])]
#[case::a_range_no_card_words(r#""choose":{"min":2,"max":4},"#, &[DRAW, GAIN, DRAW, GAIN])]
fn a_choice_no_card_words_is_not_written(#[case] choose: &str, #[case] modes: &[&str]) {
    assert_eq!(
        render_variants(&ability(&entering(choose, modes))),
        Vec::<String>::new()
    );
}
