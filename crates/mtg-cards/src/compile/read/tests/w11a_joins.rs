//! Goal: a second instruction joined with "and" ("and you scry 1", "and after this phase, there is an additional combat
//! phase") reads as the two sentences in order and writes back as the card's text, "it" names the card that is exerted
//! or attacks in "can't be blocked this turn", and an "and" before another sentence stays with the table.
use super::*;

#[rstest]
#[case::cant_be_blocked_and_scry(
    "When you do, it can't be blocked this turn and you scry 1.",
    r#"{"Triggered":{"when":{"Exerted":{"what":["This"]}},"effects":[{"Continuous":{"affected":"This","mods":["CantBeBlocked"],"duration":"UntilEndOfTurn"}},{"Scry":{}}]}}"#
)]
#[case::cant_be_blocked_alone(
    "Whenever ~ attacks, it can't be blocked this turn.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Continuous":{"affected":"This","mods":["CantBeBlocked"],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::untap_and_combat(
    "When you do, untap all other creatures you control and after this phase, there is an additional combat phase.",
    r#"{"Triggered":{"when":{"Exerted":{"what":["This"]}},"effects":[{"Untap":{"what":{"Each":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]}}},{"AddPhase":{"phases":["Combat"]}}]}}"#
)]
fn w11a_joined_instructions_read_and_write_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::and_before_a_target(
    "When you do, untap all other creatures you control and target creature can't block."
)]
#[case::split_from_a_payment(
    "Whenever ~ attacks, you may pay {5}{R}{R}. If you do, untap all attacking creatures and after this phase, there is an additional combat phase."
)]
#[case::after_without_phase("When you do, draw a card and after combat, you lose 1 life.")]
fn w11a_other_joins_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
