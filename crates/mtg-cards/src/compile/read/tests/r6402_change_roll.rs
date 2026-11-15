//! Goal: "After you roll a die, you may pay N life. If you do, increase or decrease the result by N." (with "Do this only once each turn.") and "... you may remove a +1/+1 counter from ~ ..." read into `R6402ChangeRoll` and write back as the card's text; a cost of 0, a change of 0, another player's rolls and another event stay unread or do not write back.
use super::*;

const NIGHT_SHIFT: &str = "After you roll a die, you may pay 1 life. If you do, increase or decrease the result by 1. Do this only once each turn.";
const XENOSQUIRRELS: &str = "After you roll a die, you may remove a +1/+1 counter from ~. If you do, increase or decrease the result by 1.";

#[rstest]
#[case::life_once(
    NIGHT_SHIFT,
    r#"{"Rule":{"rule":{"R6402ChangeRoll":{"who":"You","pay":{"Life":1},"by":1,"once":true}}}}"#
)]
#[case::a_counter(
    XENOSQUIRRELS,
    r#"{"Rule":{"rule":{"R6402ChangeRoll":{"who":"You","pay":{"RemoveCounter":"+1/+1"},"by":1}}}}"#
)]
#[case::more_life_more_change(
    "After you roll a die, you may pay 2 life. If you do, increase or decrease the result by 3.",
    r#"{"Rule":{"rule":{"R6402ChangeRoll":{"who":"You","pay":{"Life":2},"by":3}}}}"#
)]
#[case::a_charge_counter(
    "After you roll a die, you may remove a charge counter from ~. If you do, increase or decrease the result by 1.",
    r#"{"Rule":{"rule":{"R6402ChangeRoll":{"who":"You","pay":{"RemoveCounter":"charge"},"by":1}}}}"#
)]
fn r6402_a_change_of_a_roll_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
    let found = written_variants(&ability(json));
    assert!(found.iter().any(|w| same_text(w, text)), "{found:?}");
}

#[rstest]
#[case::no_life(
    "After you roll a die, you may pay 0 life. If you do, increase or decrease the result by 1."
)]
#[case::no_change(
    "After you roll a die, you may pay 1 life. If you do, increase or decrease the result by 0."
)]
#[case::another_event(
    "After you flip a coin, you may pay 1 life. If you do, increase or decrease the result by 1."
)]
#[case::only_increase(
    "After you roll a die, you may pay 1 life. If you do, increase the result by 1."
)]
fn r6402_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}

#[test]
fn r6402_another_players_rolls_do_not_write_back() {
    let json = r#"{"Rule":{"rule":{"R6402ChangeRoll":{"who":"Opponents","pay":{"Life":1},"by":1,"once":true}}}}"#;
    let found = written_variants(&ability(json));
    assert!(
        !found.iter().any(|w| same_text(w, NIGHT_SHIFT)),
        "{found:?}"
    );
}
