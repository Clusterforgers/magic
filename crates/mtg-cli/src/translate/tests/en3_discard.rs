//! Goal: `Discard` with `DiscardValid$` limits the cards the player may pick to those the filter selects, and a shape
//! beyond it (a random pick, a revealed hand, a filter that does not map) stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line}\n")
}

#[rstest]
#[case::creature(
    "Discard | Defined$ You | NumCards$ 1 | DiscardValid$ Creature",
    json!({"Discard": {"filter": [{"Type": "Creature"}]}})
)]
#[case::two_nonland(
    "Discard | Defined$ You | NumCards$ 2 | DiscardValid$ Card.nonLand",
    json!({"Discard": {"count": 2, "filter": [{"Not": {"Type": "Land"}}]}})
)]
#[case::any_card("Discard | Defined$ You | DiscardValid$ Card", json!({"Discard": {}}))]
fn a_discard_with_a_filter_limits_the_cards(#[case] line: &str, #[case] node: Value) {
    let script = spell(line);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(
        normal(found[0].clone()),
        normal(json!({"Spell": {"effects": [node]}}))
    );
}

#[rstest]
#[case::random(
    "Discard | Mode$ Random | DiscardValid$ Creature",
    "key:Discard.DiscardValid+Random"
)]
#[case::revealed_hand(
    "Discard | ValidTgts$ Opponent | Mode$ RevealYouChoose | DiscardValid$ Creature",
    "key:Discard.Mode=RevealYouChoose"
)]
fn a_discard_beyond_the_filter_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    let script = spell(line);
    assert_eq!(abilities(&script), Vec::<Value>::new());
    assert_eq!(gaps(&script), vec![gap.to_owned()]);
}
