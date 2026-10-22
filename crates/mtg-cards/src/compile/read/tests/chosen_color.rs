//! Goal: the wordings of the lands that choose a color as they enter (the Thriving lands, the Gates) and the lands that enter tapped with counters (the Vivid lands) read into the rules of the hand-written files, write back as the same text, and leave every near-miss to the table.
use super::*;

#[rstest]
#[case::other_than_red(
    "As ~ enters, choose a color other than red.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"Choose":{"ColorOtherThan":"Red"}}}}"#
)]
#[case::other_than_white(
    "As ~ enters, choose a color other than white.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"Choose":{"ColorOtherThan":"White"}}}}"#
)]
#[case::it_enters(
    "As it enters, choose a color other than blue.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"Choose":{"ColorOtherThan":"Blue"}}}}"#
)]
#[case::any_color("As ~ enters, choose a color.", r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"Choose":"Color"}}}"#)]
#[case::tapped_with_counters(
    "~ enters tapped with two charge counters on it.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"charge","count":2,"tapped":true}}}}"#
)]
#[case::tapped_with_one_counter(
    "~ enters tapped with a charge counter on it.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"charge","count":1,"tapped":true}}}}"#
)]
#[case::with_counters_untapped(
    "~ enters with a charge counter on it.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"charge","count":1}}}}"#
)]
#[case::tapped("~ enters tapped.", r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":"Tapped"}}"#)]
fn an_entering_wording_reads_as_the_hand_written_files_say(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::red_or_the_chosen(
    "{T}: Add {R} or one mana of the chosen color.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OrChosenColor":"Red"}}}]}}"#
)]
#[case::white_or_the_chosen(
    "{T}: Add {W} or one mana of the chosen color.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OrChosenColor":"White"}}}]}}"#
)]
#[case::the_chosen_alone(
    "{T}: Add one mana of the chosen color.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"ChosenColor":1}}}]}}"#
)]
fn a_mana_ability_with_the_chosen_color_reads_as_the_hand_written_files_say(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[rstest]
#[case::no_color("As ~ enters, choose a color other than.")]
#[case::not_a_color("As ~ enters, choose a color other than purple.")]
#[case::two_colors("As ~ enters, choose a color other than red or blue.")]
#[case::with_a_counter_kind_unknown("~ enters tapped with two fish on it.")]
#[case::tapped_then_something("~ enters tapped with.")]
fn a_near_miss_is_left_to_the_table(#[case] text: &str) {
    assert!(clause(text, ClauseKind::Static, false).is_none(), "{text}");
}

#[rstest]
#[case("{T}: Add {R} or one mana of the chosen colour.")]
#[case("{T}: Add {R} or one mana of any color.")]
#[case("{T}: Add {R} or {G} or one mana of the chosen color.")]
fn a_mana_near_miss_is_left_to_the_table(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Activated, false).is_none(),
        "{text}"
    );
}
