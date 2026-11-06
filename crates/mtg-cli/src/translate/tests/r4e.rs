//! Goal: the count words of spent mana and a promised gift, and the cost words that name a card's own mana value or an alternative cost, translate to the engine's amounts and conditions.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

#[rstest]
#[case::promised_gift_amount("Count$PromisedGift.2.1", json!({"When": {
    "condition": {"Compare": {"left": "Promised", "op": "Ge", "right": 1}}, "then": 2, "otherwise": 1}}))]
fn a_count_of_the_promised_gift_is_a_choice_between_two_amounts(
    #[case] x: &str,
    #[case] expected: Value,
) {
    let text = format!("Name:T\nTypes:Sorcery\nA:SP$ Draw | NumCards$ X\nSVar:X:{x}\n");
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(
        abilities(&text)[0]["Spell"]["effects"][0]["Draw"]["count"],
        expected
    );
}
