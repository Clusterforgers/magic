//! Goal: "put its counters on" another object is `PutCountersOf` of the object the script names, "another counter of each kind" is one of each kind on the target, a second kind of `PutCounterAll` is a second put with the first number unless it names its own, and a pick of several for counters reads its bounds; a pick of a divided count, a number of each kind other than one and a second number without a second kind stay gaps.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn effects(script: &str) -> Value {
    let found = abilities(script);
    let first = found.first().cloned().unwrap_or_default();
    first
        .get("Spell")
        .and_then(|body| body.get("effects"))
        .cloned()
        .unwrap_or_default()
}

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line}\nSVar:X:Count$xPaid\n")
}

const TO_MINE: &str = "ValidTgts$ Creature.YouCtrl | TgtPrompt$ Select target creature you control";

#[rstest]
#[case::its_counters_onto_the_target(
    format!("PutCounter | {TO_MINE} | CounterType$ EachFromSource | EachFromSource$ Self"),
    json!([{"PutCountersOf": {"on": {"Target": 0}, "of": "This"}}])
)]
#[case::the_target_named_again(
    format!("PutCounter | {TO_MINE} | Defined$ Targeted | CounterType$ EachFromSource | EachFromSource$ Self"),
    json!([{"PutCountersOf": {"on": {"Target": 0}, "of": "This"}}])
)]
#[case::another_of_each_kind(
    "PutCounter | ValidTgts$ Permanent,Player | CounterType$ ExistingCounter | EachExistingCounter$ True | CounterNum$ 1".to_owned(),
    json!([{"PutCountersOf": {"on": {"Target": 0}, "of": {"Target": 0}, "each": 1}}])
)]
#[case::a_second_kind_with_the_first_number(
    "PutCounterAll | ValidCards$ Creature.YouCtrl | CounterType$ P1P1 | CounterNum$ 5 | ValidCards2$ Planeswalker.YouCtrl | CounterType2$ LOYALTY".to_owned(),
    json!([
        {"PutCounters": {"on": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]}, "counter": "+1/+1", "count": 5}},
        {"PutCounters": {"on": {"Each": [{"Type": "Planeswalker"}, {"ControlledBy": "You"}]}, "counter": "loyalty", "count": 5}}
    ])
)]
#[case::a_second_kind_with_its_own_number(
    "PutCounterAll | ValidCards$ Creature.YouCtrl | CounterType$ P1P1 | CounterNum$ 2 | CounterType2$ Flying | CounterNum2$ 1".to_owned(),
    json!([
        {"PutCounters": {"on": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]}, "counter": "+1/+1", "count": 2}},
        {"PutCounters": {"on": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]}, "counter": {"Other": "flying"}}}
    ])
)]
#[case::up_to_two_picked(
    "PutCounter | CounterType$ P1P1 | CounterNum$ 1 | ChoiceAmount$ 2 | MinChoiceAmount$ 0 | Choices$ Soldier.YouCtrl".to_owned(),
    json!([
        {"Choose": {"from": {"Each": [{"Subtype": "Soldier"}, {"ControlledBy": "You"}]}, "count": {"min": 0, "max": 2}, "as": "card"}},
        {"PutCounters": {"on": {"Bound": "card"}, "counter": "+1/+1"}}
    ])
)]
#[case::exactly_four_picked(
    "PutCounter | Choices$ Permanent.YouDontCtrl | ChoiceAmount$ 4 | Defined$ ChosenCard | CounterType$ AIM | CounterNum$ 1".to_owned(),
    json!([
        {"Choose": {"from": {"Each": [{"Not": {"ControlledBy": "You"}}]}, "count": {"min": 4, "max": 4}, "as": "card"}},
        {"PutCounters": {"on": {"Bound": "card"}, "counter": {"Other": "aim"}}}
    ])
)]
fn g12b_counter_shapes_translate(#[case] line: String, #[case] expected: Value) {
    let script = spell(&line);
    assert_eq!(gaps(&script), Vec::<String>::new(), "{line}");
    assert_eq!(effects(&script), expected, "{line}");
}

#[rstest]
#[case::two_of_each_kind(
    "PutCounter | ValidTgts$ Permanent | CounterType$ ExistingCounter | EachExistingCounter$ True | CounterNum$ 2",
    "key:PutCounter.EachExistingCounter"
)]
#[case::one_kind_of_choice(
    "PutCounter | ValidTgts$ Permanent | CounterType$ ExistingCounter | CounterNum$ 1",
    "key:PutCounter.CounterType"
)]
#[case::a_divided_pick(
    "PutCounter | Choices$ Creature.YouCtrl | CounterType$ P1P1 | CounterNum$ X | ChoiceAmount$ X | MinChoiceAmount$ 0 | DividedAsYouChoose$ X",
    "key:PutCounter.ChoiceAmount"
)]
#[case::a_second_number_alone(
    "PutCounterAll | ValidCards$ Creature.YouCtrl | CounterType$ P1P1 | CounterNum2$ 2",
    "key:PutCounterAll.CounterType2"
)]
fn g12b_other_counter_shapes_stay_gaps(#[case] line: &str, #[case] gap: &str) {
    let found = gaps(&spell(line));
    assert!(found.iter().any(|found| found == gap), "{found:?}");
}
