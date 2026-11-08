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

#[rstest]
#[case::itself("Card.Self", Some(json!("X")))]
#[case::another_creature("Creature.YouCtrl", None)]
fn the_x_of_a_monstrosity_is_read_by_the_creature_itself(
    #[case] valid: &str,
    #[case] amount: Option<Value>,
) {
    let text = format!(
        "Name:T\nTypes:Creature Hydra\nPT:1/1\nT:Mode$ BecomeMonstrous | ValidCard$ {valid} | TriggerZones$ Battlefield | Execute$ Go | TriggerDescription$ x\nSVar:Go:DB$ GainLife | LifeAmount$ MX\nSVar:MX:TriggerCount$Amount\n"
    );
    match amount {
        Some(amount) => {
            assert_eq!(gaps(&text), Vec::<String>::new());
            let found = &abilities(&text)[0]["Triggered"];
            assert_eq!(found["effects"][0]["GainLife"]["amount"], amount);
            assert_eq!(
                found["when"],
                json!({"BecomesMonstrous": {"what": ["This"]}})
            );
        }
        None => assert_ne!(gaps(&text), Vec::<String>::new()),
    }
}
