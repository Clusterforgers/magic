//! Goal: "S faces a villainous choice — A, or B" reads for the player of the event, a target opponent and each opponent, with "that player" and "they" in the options naming the player who faces it, and writes back as the same text; another subject, one option or a choice without the dash stays with the table.
use super::*;

#[rstest]
#[case::the_player_of_the_event(
    "Whenever ~ deals combat damage to a player, that player faces a villainous choice — They sacrifice a nontoken creature of their choice, or they lose 2 life and you draw two cards.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"VillainousChoice":{"who":"Triggering","first":[{"Sacrifice":{"who":"Triggering","filter":[{"Type":"Creature"},{"Not":"Token"}]}}],"second":[{"LoseLife":{"who":"Triggering","amount":2}},{"Draw":{"count":2}}],"they":true}}]}}"#
)]
#[case::a_target_opponent(
    "Target opponent faces a villainous choice — They discard three cards, or they lose 2 life and you draw two cards.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"VillainousChoice":{"who":{"Target":0},"first":[{"Discard":{"who":{"Target":0},"count":3}}],"second":[{"LoseLife":{"who":{"Target":0},"amount":2}},{"Draw":{"count":2}}],"they":true}}]}}"#
)]
#[case::each_opponent(
    "At the beginning of your end step, each opponent faces a villainous choice — That player loses 3 life, or you draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"RepeatEach":{"over":"Opponents","as":"opponent","effects":[{"VillainousChoice":{"who":{"Bound":"opponent"},"first":[{"LoseLife":{"who":{"Bound":"opponent"},"amount":3}}],"second":[{"Draw":{}}]}}]}}]}}"#
)]
fn w2d_a_villainous_choice_reads_for_the_player_who_faces_it(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::you_face_it(
    "At the beginning of your end step, you face a villainous choice — You lose 3 life, or you draw a card.",
    ClauseKind::Triggered
)]
#[case::one_option(
    "At the beginning of your end step, each opponent faces a villainous choice — That player loses 3 life.",
    ClauseKind::Triggered
)]
#[case::no_dash(
    "At the beginning of your end step, each opponent faces a villainous choice. That player loses 3 life, or you draw a card.",
    ClauseKind::Triggered
)]
#[case::each_opponent_beside_the_player_of_the_event(
    "Whenever ~ deals combat damage to a player, each opponent faces a villainous choice — That player loses 3 life, or you draw a card.",
    ClauseKind::Triggered
)]
#[case::an_option_that_does_not_read(
    "Target opponent faces a villainous choice — They frobnicate, or you draw a card.",
    ClauseKind::Spell
)]
fn w2d_a_villainous_choice_the_table_words_differently_stays_unread(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    assert!(clause(text, kind, false).is_none(), "{text}");
}
