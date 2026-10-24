//! Goal: "For each opponent, S" and "For each player, S" read as a repeat over the players with steps that name no player of the pass, in a spell or an ability, and write back as the same text; a step that names that player, a target or a count of the players stays with the table.
use super::*;

#[rstest]
#[case::tokens_for_each_opponent(
    "For each opponent, you create a 1/1 white Human Soldier creature token.",
    ClauseKind::Spell,
    "Opponents",
    "opponent"
)]
#[case::life_for_each_opponent(
    "For each opponent, you gain 2 life.",
    ClauseKind::Spell,
    "Opponents",
    "opponent"
)]
#[case::cards_for_each_player(
    "For each player, draw a card.",
    ClauseKind::Spell,
    "EachPlayer",
    "player"
)]
#[case::in_a_trigger(
    "When this creature enters, for each opponent, you create a 1/1 white Human Soldier creature token.",
    ClauseKind::Triggered,
    "Opponents",
    "opponent"
)]
fn a_sentence_done_for_each_player_reads_as_a_repeat(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] over: &str,
    #[case] bind: &str,
) {
    let read = golden(text, kind, false);
    let json = serde_json::to_string(&read).unwrap();
    assert!(
        json.contains(&format!(r#""RepeatEach":{{"over":"{over}","as":"{bind}""#)),
        "{json}"
    );
}

#[rstest]
#[case::that_player("For each opponent, that player discards a card.", ClauseKind::Spell)]
#[case::a_target("For each opponent, destroy target creature.", ClauseKind::Spell)]
#[case::a_target_that_player_controls(
    "For each opponent, destroy target creature that player controls.",
    ClauseKind::Spell
)]
#[case::that_player_of_the_event(
    "Whenever a player casts a spell, for each opponent, that player gains 1 life.",
    ClauseKind::Triggered
)]
#[case::each_creature("For each creature, you gain 1 life.", ClauseKind::Spell)]
#[case::who_does("For each opponent who does, you gain 1 life.", ClauseKind::Spell)]
fn a_repeat_with_a_step_the_pass_cannot_bind_stays_unread(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    assert!(clause(text, kind, false).is_none(), "{text}");
}
