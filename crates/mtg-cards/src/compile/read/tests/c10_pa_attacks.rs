//! Goal: "Whenever a creature attacks you or a planeswalker you control" reads as an attack at the controller or a planeswalker of theirs with the attacker as `Triggering`, the effects after it read as on any attack at a player and write back as the card's text, and a near miss stays with the table.
use super::*;
use crate::trigger::TriggerPattern;

#[rstest]
#[case::ravens(
    "Whenever a creature attacks you or a planeswalker you control, that creature's controller loses 1 life and you gain 1 life."
)]
#[case::decree(
    "Whenever a creature attacks you or a planeswalker you control, that creature's controller loses 1 life."
)]
#[case::riddlekeeper(
    "Whenever a creature attacks you or a planeswalker you control, that creature's controller mills two cards."
)]
#[case::isperia(
    "Whenever a creature attacks you or a planeswalker you control, you may draw a card."
)]
#[case::premises("Whenever a creature attacks you or a planeswalker you control, investigate.")]
#[case::thantis(
    "Whenever a creature attacks you or a planeswalker you control, put a +1/+1 counter on ~."
)]
#[case::crown(
    "Whenever a creature attacks you or a planeswalker you control, it gets +2/+0 until end of turn."
)]
fn c10_pa_an_attack_at_you_or_your_planeswalkers_reads_and_writes_back(#[case] text: &str) {
    let read = golden(text, ClauseKind::Triggered, false);
    let Ability::Triggered { when, .. } = &read else {
        panic!("not a trigger: {read:?}");
    };
    assert!(
        matches!(
            when,
            TriggerPattern::AttacksPlayerOrPlaneswalker {
                at: crate::refs::Ref::You,
                ..
            }
        ),
        "{text}: {when:?}"
    );
}

#[rstest]
#[case::their_planeswalker(
    "Whenever a creature attacks you or a planeswalker an opponent controls, investigate."
)]
#[case::and_or("Whenever a creature attacks you and/or a planeswalker you control, investigate.")]
fn c10_pa_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
