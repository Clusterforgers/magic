//! Goal: the singular "Whenever ~ is dealt damage" and "Whenever ~ deals damage" read as the batch of damage dealt at once and write back only from it; the per-source pattern writes back only as "Whenever a source deals damage to ~"; "for the first time each turn" counts the first batch; and a wrong rule (the per-event pattern, another recipient, another count) reads `Differs`.
use super::*;

/// The clause the ability writes back, among its wordings.
fn writes(json: &str, clause: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, clause))
}

const DRAW: &str = r#""effects":[{"Draw":{}}]"#;

fn trigger(when: &str) -> String {
    format!(r#"{{"Triggered":{{"when":{when},{DRAW}}}}}"#)
}

#[rstest]
#[case::is_dealt_damage(
    "Whenever ~ is dealt damage, draw a card.",
    r#"{"Batch":{"DealsDamage":{"source":[],"to":"This"}}}"#
)]
#[case::equipped_creature_is_dealt_damage(
    "Whenever equipped creature is dealt damage, draw a card.",
    r#"{"Batch":{"DealsDamage":{"source":[],"to":"Attached"}}}"#
)]
#[case::deals_damage(
    "Whenever ~ deals damage, draw a card.",
    r#"{"Batch":{"DealsDamage":{"source":["This"]}}}"#
)]
#[case::deals_combat_damage(
    "Whenever ~ deals combat damage, draw a card.",
    r#"{"Batch":{"DealsDamage":{"source":["This"],"combat":true}}}"#
)]
#[case::the_first_time(
    "Whenever ~ is dealt damage for the first time each turn, draw a card.",
    r#"{"Nth":{"event":{"Batch":{"DealsDamage":{"source":[],"to":"This"}}},"nth":1}}"#
)]
// To a player the damage stays one event for each player (and each source), as the cards trigger.
#[case::deals_damage_to_a_player(
    "Whenever ~ deals damage to a player, draw a card.",
    r#"{"DealsDamage":{"source":["This"],"to":"EachPlayer"}}"#
)]
#[case::a_creature_you_control_is_dealt_damage(
    "Whenever a creature you control is dealt damage, draw a card.",
    r#"{"DealsDamage":{"source":[],"to":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}"#
)]
fn fix10_the_singular_wording_reads_as_the_batch(#[case] text: &str, #[case] when: &str) {
    reads_as(text, ClauseKind::Triggered, false, &trigger(when));
}

#[rstest]
#[case::blazing_sunsteel(
    "Whenever equipped creature is dealt damage, it deals that much damage to any target.",
    r#"{"Triggered":{"when":{"Batch":{"DealsDamage":{"source":[],"to":"Attached"}}},"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"source":"Attached","to":{"Target":0},"amount":"Triggering"}}]}}"#
)]
#[case::stuffy_doll(
    "Whenever ~ is dealt damage, it deals that much damage to you.",
    r#"{"Triggered":{"when":{"Batch":{"DealsDamage":{"source":[],"to":"This"}}},"effects":[{"DealDamage":{"to":"You","amount":"Triggering"}}]}}"#
)]
fn fix10_it_is_the_object_dealt_damage(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::the_per_source_pattern_is_not_is_dealt(
    r#"{"DealsDamage":{"source":[],"to":"This"}}"#,
    "Whenever ~ is dealt damage, draw a card."
)]
#[case::the_per_source_pattern_of_equipped(
    r#"{"DealsDamage":{"source":[],"to":"Attached"}}"#,
    "Whenever equipped creature is dealt damage, draw a card."
)]
#[case::the_per_recipient_pattern_is_not_deals(
    r#"{"DealsDamage":{"source":["This"]}}"#,
    "Whenever ~ deals damage, draw a card."
)]
#[case::the_per_recipient_combat_pattern(
    r#"{"DealsDamage":{"source":["This"],"combat":true}}"#,
    "Whenever ~ deals combat damage, draw a card."
)]
#[case::the_per_event_first_time(
    r#"{"Nth":{"event":{"DealsDamage":{"source":[],"to":"This"}},"nth":1}}"#,
    "Whenever ~ is dealt damage for the first time each turn, draw a card."
)]
#[case::the_second_batch(
    r#"{"Nth":{"event":{"Batch":{"DealsDamage":{"source":[],"to":"This"}}},"nth":2}}"#,
    "Whenever ~ is dealt damage for the second time each turn, draw a card."
)]
#[case::the_batch_is_not_a_source(
    r#"{"Batch":{"DealsDamage":{"source":[],"to":"This"}}}"#,
    "Whenever a source deals damage to ~, draw a card."
)]
#[case::other_recipient(
    r#"{"Batch":{"DealsDamage":{"source":[],"to":"Attached"}}}"#,
    "Whenever ~ is dealt damage, draw a card."
)]
#[case::combat_only(
    r#"{"Batch":{"DealsDamage":{"source":["This"],"combat":true}}}"#,
    "Whenever ~ deals damage, draw a card."
)]
fn fix10_a_wrong_rule_differs(#[case] when: &str, #[case] clause: &str) {
    assert!(!writes(&trigger(when), clause), "{when}");
}

#[rstest]
#[case::the_card("~", r#""This""#)]
#[case::equipped_creature("equipped creature", r#""Attached""#)]
fn fix10_the_per_source_pattern_is_a_source_dealing_damage(
    #[case] subject: &str,
    #[case] to: &str,
) {
    let when = format!(r#"{{"DealsDamage":{{"source":[],"to":{to}}}}}"#);
    let clause = format!("Whenever a source deals damage to {subject}, draw a card.");
    assert!(writes(&trigger(&when), &clause), "{clause}");
}

#[rstest]
#[case::the_equipment_deals_it(
    r#"{"Triggered":{"when":{"Batch":{"DealsDamage":{"source":[],"to":"Attached"}}},"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Triggering"}}]}}"#,
    "Whenever equipped creature is dealt damage, it deals that much damage to any target."
)]
#[case::the_per_source_event(
    r#"{"Triggered":{"when":{"DealsDamage":{"source":[],"to":"Attached"}},"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"source":"Attached","to":{"Target":0},"amount":"Triggering"}}]}}"#,
    "Whenever equipped creature is dealt damage, it deals that much damage to any target."
)]
fn fix10_a_wrong_source_differs(#[case] json: &str, #[case] clause: &str) {
    assert!(!writes(json, clause), "{json}");
}

#[rstest]
#[case::the_second_time("Whenever ~ is dealt damage for the second time each turn, draw a card.")]
fn fix10_a_later_batch_is_not_read(#[case] text: &str) {
    assert!(clause(text, ClauseKind::Triggered, false).is_none());
}
