//! Goal: "Choose three. You may choose the same mode more than once." (two to five), "Choose one or more —", "choose one that hasn't been chosen —", mode names with capitals or small words and a mode that names its X at the end read as a `Modal` with those bounds and flags, each writes back as the card's text, and every near-miss stays with the table.
use super::*;
use crate::effect::Effect;
use crate::value::Amount;

/// The bullets after a header that ends as given (with its dash or period).
fn c11d_lines(header: &str, modes: &[&str]) -> String {
    let bullets: Vec<String> = modes.iter().map(|m| format!("\n\u{2022} {m}")).collect();
    format!("{header}{}", bullets.concat())
}

const C11D_REPEAT: &str = ". You may choose the same mode more than once.";
const C11D_MODES: [&str; 3] = ["Draw a card.", "You gain 2 life.", "Scry 1."];

#[rstest]
#[case::two(c11d_lines(&format!("Choose two{C11D_REPEAT}"), &C11D_MODES), 2, Some(2), true)]
#[case::three(c11d_lines(&format!("Choose three{C11D_REPEAT}"), &C11D_MODES), 3, Some(3), true)]
#[case::five_of_two(c11d_lines(&format!("Choose five{C11D_REPEAT}"), &C11D_MODES[..2]), 5, Some(5), true)]
#[case::three_of_four(c11d_lines("Choose three \u{2014}", &["Draw a card.", "You gain 2 life.", "Scry 1.", "Scry 2."]), 3, Some(3), false)]
#[case::one_or_more(c11d_lines("Choose one or more \u{2014}", &C11D_MODES), 1, None, false)]
#[case::one_or_more_of_two(c11d_lines("Choose one or more \u{2014}", &C11D_MODES[..2]), 1, None, false)]
fn c11d_headers_read_with_their_bounds_and_write_back(
    #[case] text: String,
    #[case] min: u32,
    #[case] max: Option<i32>,
    #[case] repeats: bool,
) {
    let read = golden(&text, ClauseKind::Modal, false);
    let [Effect::Modal { choose, repeat, .. }] = read.effects() else {
        panic!("not one modal: {read:?}");
    };
    assert_eq!(choose.fixed_min(), Some(min), "{text}");
    assert_eq!(choose.max, max.map(Amount::Fixed), "{text}");
    assert_eq!(*repeat, repeats, "{text}");
}

#[rstest]
#[case::trigger(
    "At the beginning of your end step, choose one that hasn't been chosen \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 3 life.",
    ClauseKind::Triggered
)]
#[case::activated(
    "{1}, {T}: Choose one that hasn't been chosen \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 3 life.",
    ClauseKind::Activated
)]
#[case::named_modes(
    "At the beginning of your end step, choose one that hasn't been chosen \u{2014}\n\u{2022} AV Bead \u{2014} Draw a card.\n\u{2022} Prime Bead \u{2014} You gain 3 life.",
    ClauseKind::Triggered
)]
fn c11d_a_mode_not_chosen_ever_reads_and_writes_back(#[case] text: &str, #[case] kind: ClauseKind) {
    let read = golden(text, kind, false);
    let [
        Effect::Modal {
            once_ever,
            once_each_turn,
            ..
        },
    ] = read.effects()
    else {
        panic!("not one modal: {read:?}");
    };
    assert!(*once_ever && !*once_each_turn, "{text}");
}

#[rstest]
#[case::small_words("Hire a Mercenary", "Hire a Mercenary \u{2014} Draw a card.")]
#[case::capitals("AV Bead", "AV Bead \u{2014} Draw a card.")]
#[case::plain("Sell Contraband", "Sell Contraband \u{2014} Draw a card.")]
fn c11d_mode_names_read_whole(#[case] label: &str, #[case] first: &str) {
    let text = c11d_lines("Choose one or more \u{2014}", &[first, "You gain 2 life."]);
    let read = golden(&text, ClauseKind::Modal, false);
    let [Effect::Modal { modes, .. }] = read.effects() else {
        panic!("not one modal: {read:?}");
    };
    assert_eq!(modes[0].label.as_deref(), Some(label));
}

#[test]
fn c11d_a_mode_names_its_x_at_the_end() {
    let text = c11d_lines(
        "Choose one \u{2014}",
        &[
            "Put X +1/+1 counters on target creature, where X is the number of lands you control. It gains trample until end of turn.",
            "Draw a card.",
        ],
    );
    golden(&text, ClauseKind::Modal, false);
}

#[rstest]
#[case::repeat_of_one(c11d_lines(&format!("Choose one{C11D_REPEAT}"), &C11D_MODES))]
#[case::repeat_of_one_or_more(c11d_lines(&format!("Choose one or more{C11D_REPEAT}"), &C11D_MODES))]
#[case::repeat_and_a_dash(c11d_lines(&format!("Choose three{C11D_REPEAT} \u{2014}"), &C11D_MODES))]
#[case::repeat_without_period(c11d_lines("Choose three. You may choose the same mode more than once", &C11D_MODES))]
#[case::another_repeat(c11d_lines("Choose three. You may choose a mode more than once.", &C11D_MODES))]
#[case::three_of_three(c11d_lines("Choose three \u{2014}", &C11D_MODES))]
#[case::six(c11d_lines(&format!("Choose six{C11D_REPEAT}"), &C11D_MODES))]
#[case::repeat_with_one_mode(c11d_lines(&format!("Choose three{C11D_REPEAT}"), &C11D_MODES[..1]))]
#[case::one_or_more_of_one(c11d_lines("Choose one or more \u{2014}", &C11D_MODES[..1]))]
#[case::one_or_more_with_a_period(c11d_lines("Choose one or more.", &C11D_MODES))]
#[case::not_chosen_with_a_period(c11d_lines("Choose one that hasn't been chosen.", &C11D_MODES))]
#[case::two_not_chosen(c11d_lines("Choose two that haven't been chosen \u{2014}", &C11D_MODES))]
#[case::lowercase_name(c11d_lines("Choose one \u{2014}", &["hire a Mercenary \u{2014} Draw a card.", "Scry 1."]))]
#[case::small_word_at_the_end(c11d_lines("Choose one \u{2014}", &["Hire a \u{2014} Draw a card.", "Scry 1."]))]
#[case::unread_mode(c11d_lines(&format!("Choose three{C11D_REPEAT}"), &["Draw a card.", "Wish for a card."]))]
fn c11d_near_misses_stay_unread(#[case] text: String) {
    assert!(clause(&text, ClauseKind::Modal, false).is_none(), "{text}");
}
