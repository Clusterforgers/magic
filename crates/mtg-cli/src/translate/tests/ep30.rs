//! Goal: the pieces of ep30 translate as the card prints them. "Remove up to N counters" keeps the controller's stop
//! (`up_to`) for counters of any kind and of one kind; "up to all" stays a gap.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn removal(kind: &str, num: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ RemoveCounter | ValidTgts$ Permanent | CounterType$ {kind} | CounterNum$ {num} | UpTo$ True | SpellDescription$ Remove up to some counters from target permanent.\n"
    )
}

#[rstest]
#[case::any_kind("Any", "5", json!({"RemoveCounters": {"from": {"Target": 0}, "count": 5, "up_to": true}}))]
#[case::one_kind(
    "CHARGE",
    "4",
    json!({"RemoveCounters": {"from": {"Target": 0}, "counter": "charge", "count": 4, "up_to": true}})
)]
fn ep30_up_to_counters_keep_the_stop(
    #[case] kind: &str,
    #[case] num: &str,
    #[case] effect: serde_json::Value,
) {
    let script = removal(kind, num);
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(abilities(&script)[0]["Spell"]["effects"][0], effect);
}

#[test]
fn ep30_up_to_all_stays_a_gap() {
    let found = gaps(&removal("CHARGE", "All"));
    assert!(
        found.iter().any(|gap| gap.contains("RemoveCounter.UpTo")),
        "{found:?}"
    );
}
