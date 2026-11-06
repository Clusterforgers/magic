//! Goal: the Effect forms of round 4 are table rows: a remembered `CantRegenerate` static is the rule `CantBeRegenerated`
//! for the objects the effect remembers, for the duration of the effect.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Instant\nA:SP$ {line}\n")
}

fn effects(script: &str) -> Value {
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    found[0]["Spell"]["effects"].clone()
}

#[rstest]
#[case::target(
    "Effect | ValidTgts$ Creature | RememberObjects$ Targeted | ForgetOnMoved$ Battlefield | StaticAbilities$ NoRegen\nSVar:NoRegen:Mode$ CantRegenerate | ValidCard$ Card.IsRemembered",
    json!([{"Rule": {"rule": {"CantBeRegenerated": {"Target": 0}}, "duration": "UntilEndOfTurn"}}])
)]
fn a_remembered_cant_regenerate_static_is_a_rule(#[case] script: &str, #[case] expected: Value) {
    assert_eq!(effects(&spell(script)), expected);
}

#[rstest]
#[case::every_kind(
    "RemoveCounterAll | ValidCards$ Permanent | AllCounterTypes$ True",
    json!({"RemoveAllCounters": {"from": {"Each": []}}})
)]
#[case::all_of_a_kind(
    "RemoveCounterAll | ValidCards$ Creature | CounterType$ GOLD | AllCounters$ True",
    json!({"RemoveAllCounters": {"from": {"Each": [{"Type": "Creature"}]}, "counter": {"Other": "gold"}}})
)]
#[case::no_count_is_all_of_the_kind(
    "RemoveCounterAll | ValidCards$ Permanent | CounterType$ RUST",
    json!({"RemoveAllCounters": {"from": {"Each": []}, "counter": {"Other": "rust"}}})
)]
#[case::a_number_from_each(
    "RemoveCounterAll | ValidCards$ Creature.YouCtrl | CounterType$ M1M1 | CounterNum$ 1",
    json!({"RemoveCounters": {"from": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]}, "counter": "-1/-1"}})
)]
fn remove_counter_all_reads_its_count_words(#[case] line: &str, #[case] node: Value) {
    assert_eq!(effects(&spell(line)), json!([node]));
}

#[rstest]
#[case::cards_in_exile(
    "RemoveCounterAll | ValidCards$ Card.suspended | CounterType$ TIME | CounterNum$ 2 | ValidZone$ Exile",
    "key:RemoveCounterAll.ValidZone"
)]
fn remove_counter_all_leaves_what_it_cannot_read_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(line)), vec![gap.to_owned()]);
}
