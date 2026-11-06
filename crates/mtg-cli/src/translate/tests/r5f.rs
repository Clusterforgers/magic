//! Goal: the engine pieces of round 5 pack F are read from Forge scripts: freerunning with its cost is an alternative cost keyword, and a keyword cost the reader does not know stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn keyword(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nK:{line}\nA:SP$ Draw | NumCards$ 1\n")
}

#[rstest]
#[case::freerunning("Freerunning:1 B", json!({"Freerunning": "{1}{B}"}))]
#[case::freerunning_hybrid("Freerunning:U B", json!({"Freerunning": "{U}{B}"}))]
fn freerunning_is_read_with_its_cost(#[case] line: &str, #[case] expected: Value) {
    let text = keyword(line);
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text)[0]["Keyword"], expected);
}

#[rstest]
#[case::no_cost("Freerunning")]
fn freerunning_without_a_cost_stays_a_gap(#[case] line: &str) {
    assert!(!gaps(&keyword(line)).is_empty());
}
