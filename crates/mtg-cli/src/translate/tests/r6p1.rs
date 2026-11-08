//! Goal: "the Ring tempts you" is read as an effect and as a trigger (with the creature chosen as Ring-bearer when the line names it), and a trigger that names the bearer of another player, or a key the node cannot say, stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

#[rstest]
#[case::you("RingTemptsYou", json!({"RingTempts": {}}))]
#[case::opponent("RingTemptsYou | Defined$ Opponent", json!({"RingTempts": {"who": "Opponents"}}))]
fn the_effect_is_the_node(#[case] line: &str, #[case] effect: Value) {
    let script = format!("Name:T\nTypes:Sorcery\nA:SP$ {line}\n");
    assert_eq!(
        abilities(&script),
        [normal(json!({"Spell": {"effects": [effect]}}))]
    );
}

fn trigger(valid: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nT:Mode$ RingTemptsYou | {valid} | TriggerZones$ Battlefield | Execute$ G\nSVar:G:DB$ GainLife | LifeAmount$ 1\n"
    )
}

#[rstest]
#[case::you("ValidPlayer$ You", json!({"who": "You"}))]
#[case::opponent("ValidPlayer$ Opponent", json!({"who": "Opponents"}))]
#[case::your_bearer(
    "ValidCard$ Creature.YouCtrl",
    json!({"who": "You", "bearer": [{"Type": "Creature"}, {"ControlledBy": "You"}]})
)]
#[case::another_bearer(
    "ValidCard$ Creature.YouCtrl+Other",
    json!({"who": "You", "bearer": [{"Type": "Creature"}, {"ControlledBy": "You"}, "Other"]})
)]
fn the_trigger_names_who_is_tempted_and_the_bearer(#[case] valid: &str, #[case] pattern: Value) {
    let found = abilities(&trigger(valid));
    assert_eq!(found.len(), 1, "{found:?} {:?}", gaps(&trigger(valid)));
    assert_eq!(
        found[0]["Triggered"]["when"],
        json!({"RingTempts": pattern})
    );
}

#[rstest]
#[case::any_bearer("ValidCard$ Creature", "key:Trigger.RingTemptsYou.ValidCard")]
#[case::unread_key("ValidPlayer$ You | Foo$ Bar", "key:Trigger.RingTemptsYou.Foo")]
fn what_the_node_cannot_say_is_a_gap(#[case] valid: &str, #[case] gap: &str) {
    assert!(
        gaps(&trigger(valid)).iter().any(|found| found == gap),
        "{:?}",
        gaps(&trigger(valid))
    );
}
