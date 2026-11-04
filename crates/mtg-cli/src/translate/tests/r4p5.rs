//! Goal: round 4 piece 5 translates the activated-ability cost reduction and stays a gap on a value it cannot count.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn land(reduce: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact\nA:AB$ Draw | Cost$ 3 U | NumCards$ 1 | Defined$ You | ReduceCost$ {reduce}\nSVar:X:Count$Valid Creature.YouCtrl\n"
    )
}

#[rstest]
#[case::svar("X")]
#[case::inline("Count$Valid Creature.YouCtrl")]
fn a_counted_reduction_is_a_changed_cost_item(#[case] reduce: &str) {
    assert_eq!(gaps(&land(reduce)), Vec::<String>::new());
    let found = abilities(&land(reduce));
    let cost = &found[0]["Activated"]["cost"];
    assert_eq!(cost[1]["Changed"]["change"], json!("Reduce"));
    assert_eq!(cost[1]["Changed"]["mana"], json!("{1}"));
    assert!(cost[1]["Changed"]["per"].is_object());
}

#[test]
fn a_fixed_reduction_is_a_gap() {
    assert_eq!(gaps(&land("2")), ["key:AB.ReduceCost=fixed"]);
}
