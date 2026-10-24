//! Goal: "populate" read with the sentence after it that makes the token enter tapped and attacking, or gives it keywords for good and sacrifices it at the next end step, reads into the two steps of populate with the token made to match, writes back as the same text, and leaves every other sentence after populate to the table.
use super::*;

#[rstest]
#[case::enters_tapped_and_attacking(
    "Whenever ~ attacks, populate. The token enters tapped and attacking.",
    ClauseKind::Triggered,
    r#""tapped":true,"attacking":true"#
)]
#[case::gains_haste_and_is_sacrificed(
    "At the beginning of combat on your turn, populate. The token created this way gains haste. Sacrifice it at the beginning of the next end step.",
    ClauseKind::Triggered,
    r#""as":"tokens""#
)]
#[case::gains_two_keywords(
    "At the beginning of combat on your turn, populate. The token created this way gains flying and haste. Sacrifice it at the beginning of the next end step.",
    ClauseKind::Triggered,
    r#""AddKeyword":"Flying""#
)]
#[case::in_a_spell(
    "Populate. The token enters tapped and attacking.",
    ClauseKind::Spell,
    r#""CopyOf":{"of":{"Bound":"token"}}"#
)]
fn populate_with_the_sentences_that_act_on_its_token(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] part: &str,
) {
    let read = golden(text, kind, false);
    let json = serde_json::to_string(&read).unwrap_or_default();
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::tapped_only("Whenever ~ attacks, populate. The token enters tapped.")]
#[case::attacking_only("Whenever ~ attacks, populate. The token enters attacking.")]
#[case::several_tokens("Whenever ~ attacks, populate. The tokens enter tapped and attacking.")]
#[case::at_a_player(
    "Whenever ~ attacks, populate. The token enters tapped and attacking that player."
)]
#[case::haste_without_an_end(
    "At the beginning of combat on your turn, populate. The token created this way gains haste."
)]
#[case::haste_until_end_of_turn(
    "At the beginning of combat on your turn, populate. The token created this way gains haste until end of turn. Sacrifice it at the beginning of the next end step."
)]
#[case::an_exile_in_place_of_the_sacrifice(
    "At the beginning of combat on your turn, populate. The token created this way gains haste. Exile it at the beginning of the next end step."
)]
#[case::another_step(
    "At the beginning of combat on your turn, populate. The token created this way gains haste. Sacrifice it at the beginning of the next upkeep."
)]
#[case::a_pump_in_place_of_the_keyword(
    "At the beginning of combat on your turn, populate. The token created this way gets +1/+1. Sacrifice it at the beginning of the next end step."
)]
fn what_follows_populate_in_another_shape_stays_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
