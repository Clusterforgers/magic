//! Goal: the trigger modes that restate a known event (batches of "one or more", cast or copy, unblocked) map to its
//! pattern, and a key the pattern cannot say makes the trigger a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn card(trigger: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Elf\nT:{trigger} | Execute$ Draw1\nSVar:Draw1:DB$ Draw | NumCards$ 1\n"
    )
}

#[rstest]
#[case::unblocked("Mode$ AttackerUnblocked | ValidCard$ Card.Self", json!({"Unblocked": {"what": ["This"]}}))]
#[case::cast_or_copy(
    "Mode$ SpellCastOrCopy | ValidCard$ Instant,Sorcery | ValidActivatingPlayer$ You",
    json!({"AnyOf": [
        {"SpellCast": {"by": "You", "what": [{"AnyOf": [{"Type": "Instant"}, {"Type": "Sorcery"}]}]}},
        {"SpellCopied": {"by": "You", "what": [{"AnyOf": [{"Type": "Instant"}, {"Type": "Sorcery"}]}]}}]})
)]
#[case::discards("Mode$ DiscardedAll | ValidPlayer$ You | ValidCard$ Card", json!({"Batch": {"Discarded": {"by": "You"}}}))]
#[case::life_lost("Mode$ LifeLostAll | ValidPlayer$ Opponent", json!({"Batch": {"LifeLost": {"who": "Opponents"}}}))]
#[case::sacrificed("Mode$ SacrificedOnce | ValidCard$ Creature | ValidPlayer$ You", json!({"Batch": {"Sacrificed": {"what": [{"Type": "Creature"}], "by": "You"}}}))]
#[case::damage(
    "Mode$ DamageAll | CombatDamage$ True | ValidSource$ Creature.YouCtrl | ValidTarget$ Player",
    json!({"Batch": {"DealsDamage": {"source": [{"Type": "Creature"}, {"ControlledBy": "You"}], "to": "EachPlayer", "combat": true}}})
)]
#[case::counters(
    "Mode$ CounterAddedOnce | ValidCard$ Card.Self | CounterType$ P1P1",
    json!({"Batch": {"CountersPut": {"what": ["This"], "counter": "+1/+1"}}})
)]
fn a_mode_maps_to_its_pattern(#[case] trigger: &str, #[case] when: Value) {
    let script = card(trigger);
    let mut found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let want = json!({"Triggered": {"when": when, "effects": [{"Draw": {}}]}});
    assert_eq!(normal(found.remove(0)), normal(want));
}

#[rstest]
#[case::first_time(
    "Mode$ DiscardedAll | ValidPlayer$ You | FirstTime$ True",
    "key:Trigger.DiscardedAll.FirstTime"
)]
#[case::amount_each(
    "Mode$ LifeLostAll | ValidPlayer$ Opponent | ValidAmountEach$ EQ1",
    "key:Trigger.LifeLostAll.ValidAmountEach"
)]
#[case::source(
    "Mode$ CounterAddedOnce | ValidCard$ Creature | ValidSource$ Card.Red",
    "key:Trigger.CounterAddedOnce.ValidSource=Card.Red"
)]
#[case::defender(
    "Mode$ AttackerUnblocked | ValidCard$ Card.Self | ValidDefender$ You",
    "key:Trigger.AttackerUnblocked.ValidDefender"
)]
fn an_unread_key_stays_a_gap(#[case] trigger: &str, #[case] gap: &str) {
    let script = card(trigger);
    assert_eq!(abilities(&script), Vec::<Value>::new(), "{script}");
    assert!(
        gaps(&script).iter().any(|found| found == gap),
        "{:?}",
        gaps(&script)
    );
}
