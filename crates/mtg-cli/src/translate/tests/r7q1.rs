//! Goal: the fixes of the round 7 audit read their Forge keys right: the nth counter compares the total on the object, and a shape the DSL cannot say stays a named gap.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn trigger(line: &str) -> String {
    format!(
        "Name:T\nTypes:Enchantment\nT:Mode$ {line} | Execute$ Go | TriggerZones$ Battlefield | TriggerDescription$ x\nSVar:Go:DB$ GainLife | LifeAmount$ 1\n"
    )
}

#[rstest]
#[case::on_itself(
    "CounterAdded | ValidCard$ Card.Self | CounterType$ P1P1 | CounterAmount$ EQ3",
    Some(json!({"AtTrigger": {"Compare": {"left": {"Counters": {"on": "This", "counter": "+1/+1"}}, "op": "Eq", "right": 3}}}))
)]
#[case::on_another_creature_waits(
    "CounterAdded | ValidCard$ Creature | CounterType$ P1P1 | CounterAmount$ EQ3",
    None
)]
#[case::of_no_kind_waits("CounterAdded | ValidCard$ Card.Self | CounterAmount$ EQ3", None)]
fn the_nth_counter_compares_the_total(#[case] mode: &str, #[case] condition: Option<Value>) {
    let text = trigger(mode);
    match condition {
        Some(condition) => {
            assert_eq!(gaps(&text), Vec::<String>::new());
            assert_eq!(abilities(&text)[0]["Triggered"]["if"], condition);
        }
        None => assert!(
            gaps(&text)
                .iter()
                .any(|gap| gap.starts_with("key:Trigger.CounterAmount")),
            "{:?}",
            gaps(&text)
        ),
    }
}
