//! Goal: round 3 pack F (costs and mana that need the engine) translates each construct to the rule it names, one rstest table per rule.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn creature(lines: &str) -> String {
    format!("Name:T\nManaCost:2\nTypes:Creature Eldrazi\nPT:1/1\n{lines}\n")
}

#[rstest]
#[case::two_opponents_cards(
    "A:AB$ Draw | Cost$ ExiledMoveToGrave<2/Card.OppOwn/cards your opponents own> | NumCards$ 1",
    json!([{"ExiledToGraveyard": {"count": 2, "filter": [{"OwnedBy": "Opponents"}]}}])
)]
#[case::one_exiled_with_the_source(
    "A:AB$ Draw | Cost$ ExiledMoveToGrave<1/Card.ExiledWithSource/card exiled with CARDNAME> | NumCards$ 1",
    json!([{"ExiledToGraveyard": {"filter": [{"ExiledWith": "This"}]}}])
)]
fn exiled_cards_put_into_graveyards_are_a_cost(#[case] line: &str, #[case] cost: Value) {
    let text = creature(line);
    let found = abilities(&text);
    assert!(!found.is_empty(), "{:?}", gaps(&text));
    assert_eq!(found[0]["Activated"]["cost"], cost);
}
