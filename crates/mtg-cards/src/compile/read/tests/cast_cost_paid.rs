//! Goal: "if its prowl cost was paid" and "if its mayhem cost was paid" read as the condition `CastWith` of the way on an enters trigger and "If this spell's prowl cost was paid, ..." as an `Effect::If` after a spell's effects, each written back as the same text, and a cost the template does not model stays with the table.
use super::*;

fn spell_of(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Spell, false)
}

#[rstest]
#[case::draw_when_it_enters(
    "When this creature enters, if its prowl cost was paid, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"CastWith":"Prowl"},"effects":[{"Draw":{}}]}}"#
)]
#[case::a_spell_after_its_effects(
    "Target player loses 3 life and you gain 3 life. If this spell's prowl cost was paid, draw a card.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":3}},{"GainLife":{"amount":3}},{"If":{"condition":{"CastWith":"Prowl"},"then":[{"Draw":{}}]}}]}}"#
)]
#[case::mayhem_when_it_enters(
    "When this creature enters, if its mayhem cost was paid, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"CastWith":"Mayhem"},"effects":[{"Draw":{}}]}}"#
)]
#[case::mayhem_after_the_effects_of_a_spell(
    "Target player loses 3 life and you gain 3 life. If this spell's mayhem cost was paid, draw a card.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":3}},{"GainLife":{"amount":3}},{"If":{"condition":{"CastWith":"Mayhem"},"then":[{"Draw":{}}]}}]}}"#
)]
#[case::kicked_with_its_first_kicker(
    "When this creature enters, if it was kicked with its {W} kicker, you gain 3 life.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"KickedWith":"{W}"},"effects":[{"GainLife":{"amount":3}}]}}"#
)]
#[case::kicked_with_its_second_kicker(
    "When this creature enters, if it was kicked with its {2}{B} kicker, you gain 3 life.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"KickedWith":"{2}{B}"},"effects":[{"GainLife":{"amount":3}}]}}"#
)]
fn the_cost_paid_condition_reads_and_writes_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::another_cost("If this spell's kicker cost was paid, draw a card.")]
#[case::the_wrong_owner("If its prowl cost was paid, draw a card.")]
#[case::no_cost("If this spell's prowl was paid, draw a card.")]
#[case::a_kicker_without_a_cost("If this spell was kicked with its kicker, draw a card.")]
fn a_condition_the_template_does_not_model_is_left_to_the_table(#[case] text: &str) {
    assert!(spell_of(text).is_none(), "{text}");
}
