//! Goal: "put a -1/-1 counter on a creature you control" as a cost reads as blight 1 and writes back as the card's text; another counter, another count or another creature stays unread.
use super::*;

#[rstest]
#[case::with_mana("{B}, Put a -1/-1 counter on a creature you control: Draw a card.")]
#[case::with_mana_and_tap(
    "{2}{R}, {T}, Put a -1/-1 counter on a creature you control: You gain 2 life."
)]
fn g12d_a_minus_counter_on_your_creature_reads_as_blight(#[case] text: &str) {
    let json = serde_json::to_string(&golden(text, ClauseKind::Activated, false)).unwrap();
    assert!(json.contains(r#"{"Blight":1}"#), "{text}: {json}");
}

#[rstest]
#[case::a_plus_counter("{1}, Put a +1/+1 counter on a creature you control: Draw a card.")]
#[case::two_counters("{1}, Put two -1/-1 counters on a creature you control: Draw a card.")]
#[case::an_opponents("{1}, Put a -1/-1 counter on a creature an opponent controls: Draw a card.")]
#[case::any_creature("{1}, Put a -1/-1 counter on a creature: Draw a card.")]
fn g12d_other_counter_costs_stay_unread(#[case] text: &str) {
    let json = clause(text, ClauseKind::Activated, false)
        .map(|read| serde_json::to_string(&read).unwrap());
    assert!(
        !json.as_deref().is_some_and(|json| json.contains("Blight")),
        "{text}: {json:?}"
    );
}
