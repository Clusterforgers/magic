//! Goal: the wordings of extra turns and added phases ("Take an extra turn after this one.", "Target player takes two extra turns after this one.", "Skip the untap step of that turn.", "After this main phase, there is an additional combat phase followed by an additional main phase.", "After this phase, there is an additional combat phase.") read into the DSL, write back as the card's own text, and leave the near-misses to the table.
use super::*;

#[rstest]
#[case::mine(
    "Take an extra turn after this one.",
    r#"{"Spell":{"effects":[{"ExtraTurn":{}}]}}"#
)]
#[case::two_of_mine(
    "Take two extra turns after this one.",
    r#"{"Spell":{"effects":[{"ExtraTurn":{"count":2}}]}}"#
)]
#[case::without_the_untap_step(
    "Take an extra turn after this one. Skip the untap step of that turn.",
    r#"{"Spell":{"effects":[{"ExtraTurn":{"skip_untap":true}}]}}"#
)]
#[case::of_a_target_player(
    "Target player takes an extra turn after this one.",
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"ExtraTurn":{"who":{"Target":0}}}]}}"#
)]
#[case::two_of_a_target_player(
    "Target player takes two extra turns after this one.",
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"ExtraTurn":{"who":{"Target":0},"count":2}}]}}"#
)]
#[case::after_another_effect(
    "Draw a card. Take an extra turn after this one.",
    r#"{"Spell":{"effects":[{"Draw":{}},{"ExtraTurn":{}}]}}"#
)]
fn a_spell_gives_extra_turns(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::combat_and_main(
    "After this main phase, there is an additional combat phase followed by an additional main phase.",
    r#"{"Spell":{"effects":[{"AddPhase":{"phases":["Combat","Main"]}}]}}"#
)]
#[case::two_combats(
    "After this main phase, there are two additional combat phases.",
    r#"{"Spell":{"effects":[{"AddPhase":{"phases":["Combat","Combat"]}}]}}"#
)]
#[case::a_combat(
    "After this phase, there is an additional combat phase.",
    r#"{"Spell":{"effects":[{"AddPhase":{"phases":["Combat"]}}]}}"#
)]
fn a_spell_adds_phases(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::the_game_takes_one("The game takes an extra turn after this one.")]
#[case::no_count("Take extra turns after this one.")]
#[case::nothing_after_it("Take an extra turn.")]
#[case::a_turn_before("Take an extra turn before this one.")]
#[case::a_count_of_coins("Take an extra turn after this one for each coin that comes up heads.")]
#[case::another_step("Take an extra turn after this one. Skip the draw step of that turn.")]
#[case::an_end_step("After this phase, there is an additional end step.")]
#[case::another_kind("After this phase, there is an additional upkeep phase.")]
#[case::a_restriction(
    "After this main phase, there is an additional combat phase. Only the chosen creatures can attack during that combat phase."
)]
#[case::the_wrong_count("After this phase, there is two additional combat phase.")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Spell, false), None);
}
