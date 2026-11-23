//! Goal: "defending player" is the subject of what an attacking card's trigger makes that player do (lose life, mill, exile the top of their library), another player exiles the top of their own library, each wording writes back as the same text, and "defending player" outside an attack trigger stays unread.
use super::*;

#[rstest]
#[case::defending_exiles_the_top(
    "Whenever ~ attacks, defending player exiles the top twenty cards of their library.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"ExileTop":{"who":"Defending","count":20,"as":"exiled"}}]}}"#
)]
#[case::defending_exiles_the_top_card(
    "Whenever ~ attacks, defending player exiles the top card of their library.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"ExileTop":{"who":"Defending","as":"exiled"}}]}}"#
)]
#[case::target_opponent_exiles_the_top(
    "Whenever ~ attacks, target opponent exiles the top two cards of their library.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":"Opponent"}],"effects":[{"ExileTop":{"who":{"Target":0},"count":2,"as":"exiled"}}]}}"#
)]
#[case::each_opponent_exiles_the_top(
    "Whenever ~ attacks, each opponent exiles the top three cards of their library.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"ExileTop":{"who":"Opponents","count":3,"as":"exiled"}}]}}"#
)]
#[case::defending_loses_life(
    "Whenever ~ attacks, defending player loses 1 life.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"LoseLife":{"who":"Defending","amount":1}}]}}"#
)]
#[case::defending_mills(
    "Whenever ~ attacks, defending player mills two cards.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Mill":{"who":"Defending","count":2}}]}}"#
)]
fn another_player_acts_on_their_own_cards(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::defending_outside_an_attack("When ~ enters, defending player loses 1 life.")]
#[case::defending_on_a_cast("Whenever you cast a spell, defending player mills two cards.")]
#[case::your_library_named_theirs(
    "Whenever ~ attacks, defending player exiles the top two cards of your library."
)]
fn the_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}

#[test]
fn each_opponent_exiles_the_top_after_other_verbs_in_one_sentence() {
    reads_as(
        "When ~ enters, each opponent discards a card, loses 2 life, and exiles the top three cards of their library.",
        ClauseKind::Triggered,
        false,
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Discard":{"who":"Opponents"}},{"LoseLife":{"who":"Opponents","amount":2}},{"ExileTop":{"who":"Opponents","count":3,"as":"exiled"}}]}}"#,
    );
}

#[rstest]
#[case::target_opponent(
    r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"ExileTop":{"who":{"Target":0},"count":3,"as":"exiled"}}]}}"#,
    "Exile the top three cards of target opponent's library."
)]
#[case::target_player(
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"ExileTop":{"who":{"Target":0},"as":"exiled"}}]}}"#,
    "Exile the top card of target player's library."
)]
fn a_target_player_exiles_the_top_in_the_wording_the_cards_print(
    #[case] json: &str,
    #[case] text: &str,
) {
    let wordings = render_variants(&ability(json));
    assert!(
        wordings.iter().any(|written| same_text(written, text)),
        "{wordings:?}"
    );
}
