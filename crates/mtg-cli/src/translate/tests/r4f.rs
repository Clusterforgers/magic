//! Goal: each Forge parameter form of the r4f pack reads to the effect it spells, probed by a card per form; forms the
//! engine cannot state stay named gaps.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn first_effects(script: &str) -> Value {
    let ability = abilities(script).into_iter().next().unwrap();
    let (_, body) = ability.as_object().unwrap().iter().next().unwrap();
    body["effects"].clone()
}

#[rstest]
#[case::different_names(
    "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Library | Destination$ Hand | ChangeType$ Aura | ChangeNum$ 3 | DifferentNames$ True\n",
    true
)]
#[case::any_names(
    "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Library | Destination$ Hand | ChangeType$ Aura | ChangeNum$ 3\n",
    false
)]
fn a_search_with_different_names_finds_one_card_per_name(
    #[case] script: &str,
    #[case] distinct: bool,
) {
    let effects = first_effects(script);
    assert_eq!(
        effects[0]["Search"].get("distinct"),
        distinct.then(|| &json!(true))
    );
    assert!(gaps(script).is_empty(), "{:?}", gaps(script));
}
