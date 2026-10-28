//! Goal: a counter of any kind reads as a cost part from a permanent the payer controls and as an effect with or without "up to", and writes back as the card's text; a counter of any kind from the source, more than one from a permanent, "up to a counter", X and another player's permanent stay unread.
use super::*;

#[rstest]
#[case::power_conduit(
    "{T}, Remove a counter from a permanent you control: Draw a card.",
    r#"{"RemoveCounters":{"from":[]}}"#
)]
#[case::fain(
    "{T}, Remove a counter from a creature you control: Create a Treasure token.",
    r#"{"RemoveCounters":{"from":[{"Type":"Creature"}]}}"#
)]
#[case::hexavus(
    "{1}, Remove a counter from another creature you control: Put a +1/+1 counter on ~.",
    r#"{"RemoveCounters":{"from":[{"Type":"Creature"},"Other"]}}"#
)]
#[case::nonland(
    "{T}, Remove a counter from a nonland permanent you control: Draw a card.",
    r#"{"RemoveCounters":{"from":[{"Not":{"Type":"Land"}}]}}"#
)]
fn w2f_any_counter_costs_read_and_write_back(#[case] text: &str, #[case] part: &str) {
    let json = serde_json::to_string(&golden(text, ClauseKind::Activated, false)).unwrap();
    assert!(json.contains(part), "{text}: {json}");
}

#[rstest]
#[case::up_to_three(
    "Remove up to three counters from target permanent.",
    r#"{"Spell":{"targets":[{"what":{"Object":[]}}],"effects":[{"RemoveCounters":{"from":{"Target":0},"count":3,"up_to":true}}]}}"#
)]
#[case::a_counter(
    "Remove a counter from target permanent.",
    r#"{"Spell":{"targets":[{"what":{"Object":[]}}],"effects":[{"RemoveCounters":{"from":{"Target":0}}}]}}"#
)]
#[case::from_the_card(
    "Remove a counter from ~.",
    r#"{"Spell":{"effects":[{"RemoveCounters":{"from":"This"}}]}}"#
)]
#[case::up_to_of_a_kind(
    "Remove up to four charge counters from target permanent.",
    r#"{"Spell":{"targets":[{"what":{"Object":[]}}],"effects":[{"RemoveCounters":{"from":{"Target":0},"counter":"charge","count":4,"up_to":true}}]}}"#
)]
fn w2f_any_counter_effects_read_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::from_the_source("{T}, Remove a counter from ~: Draw a card.", ClauseKind::Activated)]
#[case::two_from_a_permanent(
    "{T}, Remove two counters from a creature you control: Draw a card.",
    ClauseKind::Activated
)]
#[case::an_opponents(
    "{T}, Remove a counter from a creature an opponent controls: Draw a card.",
    ClauseKind::Activated
)]
#[case::up_to_a_counter("Remove up to a counter from target permanent.", ClauseKind::Spell)]
#[case::x_counters("Remove X counters from target permanent.", ClauseKind::Spell)]
fn w2f_any_counter_near_misses_stay_unread(#[case] text: &str, #[case] kind: ClauseKind) {
    let json = clause(text, kind, false).map(|read| serde_json::to_string(&read).unwrap());
    assert!(
        !json
            .as_deref()
            .is_some_and(|json| json.contains(r#""RemoveCounters""#)),
        "{text}: {json:?}"
    );
}
