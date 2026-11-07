//! Goal: pack e14d's mechanics read from Forge lines into the engine's one model of each: recruit expands its keyword action, and a line with a key the mechanic does not read stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn plain(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line}\n")
}

fn effects(text: &str) -> Vec<Value> {
    abilities(text)[0]["Spell"]["effects"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

#[test]
fn recruit_is_the_keyword_action_of_the_table() {
    let steps = mtg_cards::macros::action_effects("recruit", 0, &mtg_cards::refs::Ref::You)
        .unwrap_or_default();
    let expected: Vec<Value> = steps.iter().map(|step| json!(step)).collect();
    assert_eq!(gaps(&plain("Recruit")), Vec::<String>::new());
    assert_eq!(effects(&plain("Recruit")), expected);
}

#[rstest]
#[case::a_number("Recruit | Num$ 2")]
#[case::another_player("Recruit | Defined$ Opponent")]
fn recruit_with_a_key_it_does_not_read_is_a_gap(#[case] line: &str) {
    assert_ne!(gaps(&plain(line)).len(), 0);
}
