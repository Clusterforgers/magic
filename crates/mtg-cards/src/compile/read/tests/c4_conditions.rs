//! Goal: the phase of the turn in a condition ("if it's the first combat phase of the turn", "... of your turn", "if it's
//! your main phase", "if it's your combat phase"), the creatures that attacked this turn and the rules that stop searches
//! and skip extra turns read into the DSL and write back as the same text; another phase, turn, group or verb stays with
//! the table.
use super::*;

#[rstest]
#[case::the_first_combat(
    "Whenever equipped creature attacks, if it's the first combat phase of the turn, untap all attacking creatures. After this phase, there is an additional combat phase.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Attacks":{"what":[{"Is":"Attached"}]}},"if":"FirstCombat","effects":[{"Untap":{"what":{"Each":[{"Type":"Creature"},"Attacking"]}}},{"AddPhase":{"phases":["Combat"]}}]}}"#
)]
#[case::a_main_phase_of_yours(
    "When this Aura enters, if it's your main phase, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"All":["YourTurn",{"Any":[{"Step":"Main1"},{"Step":"Main2"}]}]},"effects":[{"Draw":{}}]}}"#
)]
#[case::a_combat_phase_of_yours(
    "When this creature enters, if it's your combat phase, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"All":["YourTurn","InCombat"]},"effects":[{"Draw":{}}]}}"#
)]
#[case::the_first_combat_of_your_turn(
    "When this creature enters, if it's the first combat phase of your turn, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"All":["YourTurn","FirstCombat"]},"effects":[{"Draw":{}}]}}"#
)]
#[case::the_creatures_that_attacked(
    "Untap all creatures that attacked this turn.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Untap":{"what":{"Each":[{"Type":"Creature"},"AttackedThisTurn"]}}}]}}"#
)]
#[case::relentless_assault(
    "Untap all creatures that attacked this turn. After this main phase, there is an additional combat phase followed by an additional main phase.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Untap":{"what":{"Each":[{"Type":"Creature"},"AttackedThisTurn"]}}},{"AddPhase":{"phases":["Combat","Main"]}}]}}"#
)]
#[case::opponents_cant_search(
    "Your opponents can't search libraries.",
    ClauseKind::Static,
    r#"{"Rule":{"rule":{"CantSearch":"Opponents"}}}"#
)]
#[case::players_cant_search(
    "Players can't search libraries.",
    ClauseKind::Static,
    r#"{"Rule":{"rule":{"CantSearch":"EachPlayer"}}}"#
)]
#[case::an_opponent_skips(
    "If an opponent would begin an extra turn, that player skips that turn instead.",
    ClauseKind::Static,
    r#"{"Rule":{"rule":{"SkipExtraTurns":"Opponents"}}}"#
)]
#[case::a_player_skips(
    "If a player would begin an extra turn, that player skips that turn instead.",
    ClauseKind::Static,
    r#"{"Rule":{"rule":{"SkipExtraTurns":"EachPlayer"}}}"#
)]
fn a_phase_condition_a_set_of_attackers_and_a_turn_rule_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::the_second_combat(
    "When this creature enters, if it's the second combat phase of the turn, draw a card.",
    ClauseKind::Triggered
)]
#[case::an_end_step(
    "When this creature enters, if it's your end step, draw a card.",
    ClauseKind::Triggered
)]
#[case::an_opponents_main_phase(
    "When this creature enters, if it's an opponent's main phase, draw a card.",
    ClauseKind::Triggered
)]
#[case::the_first_main_phase_of_the_turn(
    "When this creature enters, if it's the first main phase of the turn, draw a card.",
    ClauseKind::Triggered
)]
#[case::attacked_last_turn("Untap all creatures that attacked last turn.", ClauseKind::Spell)]
#[case::blocked_this_turn("Untap all creatures that blocked this turn.", ClauseKind::Spell)]
#[case::you_skip(
    "If you would begin an extra turn, you skip that turn instead.",
    ClauseKind::Static
)]
#[case::another_turn(
    "If an opponent would begin an extra turn, that player skips the next turn instead.",
    ClauseKind::Static
)]
#[case::a_quality_of_the_extra_turn(
    "If an opponent would begin an extra turn, that player skips that turn's draw step instead.",
    ClauseKind::Static
)]
#[case::their_searches("Your opponents can't search their libraries.", ClauseKind::Static)]
#[case::one_opponent("Target opponent can't search libraries.", ClauseKind::Static)]
fn near_misses_of_the_phase_conditions_and_turn_rules_stay_with_the_table(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
