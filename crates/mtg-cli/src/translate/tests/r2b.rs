//! Goal: a trigger line that names who caused the event wraps its pattern in `Caused` with the players the word names, and a cause word the engine cannot judge stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn line(trigger: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Human\nPT:2/2\nT:{trigger} | TriggerZones$ Battlefield | Execute$ Go | TriggerDescription$ x\nSVar:Go:DB$ GainLife | LifeAmount$ 1\n"
    )
}

#[rstest]
#[case::a_counter_you_put(
    "Mode$ CounterAdded | ValidCard$ Creature.Other | ValidSource$ You | CounterType$ P1P1",
    json!({"Caused": {"inner": {"CountersPut": {"what": [{"Type": "Creature"}, "Other"], "counter": "+1/+1"}}, "by": "You"}})
)]
#[case::counters_you_put_together(
    "Mode$ CounterAddedOnce | ValidCard$ Creature | ValidSource$ You | CounterType$ M1M1",
    json!({"Caused": {"inner": {"Batch": {"CountersPut": {"what": [{"Type": "Creature"}], "counter": "-1/-1"}}}, "by": "You"}})
)]
#[case::discarded_by_an_opponents_spell(
    "Mode$ Discarded | ValidCard$ Card.Self | ValidCause$ SpellAbility.OppCtrl",
    json!({"Caused": {"inner": {"Discarded": {"what": ["This"]}}, "by": "Opponents"}})
)]
fn a_cause_word_wraps_the_event(#[case] trigger: &str, #[case] when: Value) {
    let script = line(trigger);
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(abilities(&script)[0]["Triggered"]["when"], when);
}

#[test]
fn a_cause_that_is_an_object_filter_stays_a_gap() {
    let script = line("Mode$ Discarded | ValidCard$ Card.Self | ValidCause$ Card.Red");
    assert_eq!(
        gaps(&script),
        vec!["key:Trigger.Discarded.ValidCause=Card.Red".to_owned()]
    );
}

#[rstest]
#[case::blocked_this_turn("blockedThisTurn", r#"{"DidThisTurn":{"event":"Blocked"}}"#)]
#[case::blocked_this_combat(
    "blockedThisCombat",
    r#"{"DidThisTurn":{"event":"Blocked","combat":true}}"#
)]
#[case::attacked_this_combat(
    "attackedThisCombat",
    r#"{"DidThisTurn":{"event":"Attacked","combat":true}}"#
)]
#[case::dealt_damage_this_turn("dealtDamageThisTurn", r#"{"DidThisTurn":{"event":"Damaged"}}"#)]
#[case::dealt_damage_to_you(
    "dealtDamageToYouThisTurn",
    r#"{"DidThisTurn":{"event":"Damaged","to":"You"}}"#
)]
fn a_property_of_the_turns_log_is_a_did_this_turn_predicate(
    #[case] property: &str,
    #[case] expected: &str,
) {
    let script =
        format!("Name:T\nTypes:Instant\nA:SP$ Pump | ValidTgts$ Creature.{property} | NumAtt$ 1\n");
    assert_eq!(gaps(&script), Vec::<String>::new());
    let expected: Value = serde_json::from_str(expected).unwrap();
    let found = &abilities(&script)[0]["Spell"]["targets"][0]["what"]["Object"][1];
    assert_eq!(*found, expected);
}

#[rstest]
#[case::any_kind_of_counter("Any You Creature", true)]
#[case::a_kind_of_counter("P1P1 You Creature", false)]
fn counters_put_this_turn_count_only_without_a_kind(#[case] words: &str, #[case] read: bool) {
    let script = format!(
        "Name:T\nTypes:Creature Human\nPT:1/1\nS:Mode$ Continuous | Affected$ Card.Self | AddPower$ X\nSVar:X:Count$CountersAddedThisTurn {words}\n"
    );
    assert_eq!(gaps(&script).is_empty(), read, "{:?}", gaps(&script));
    if read {
        let found = serde_json::to_string(&abilities(&script)).unwrap();
        assert!(found.contains(r#""event":"CountersPut""#), "{found}");
    }
}
