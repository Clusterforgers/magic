//! Goal: "with mana cost {N}" and "with mana cost {N} or {M}" after a searched card read as costs of exactly that generic mana and are written back, a quoted ability inside a granted one is written in single quotes, and the search of Urza's Saga's chapter III compiles from its printed text to its rules.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use mtg_cards::ability::Ability;
use mtg_cards::card::Card;
use mtg_cards::compile::{compile, same_written_text, written_variants};
use rstest::rstest;

/// A spell that searches for a card with this filter and puts it onto the battlefield.
fn search(filter: &str) -> Ability {
    serde_json::from_str(&format!(
        r#"{{"Spell":{{"effects":[{{"Search":{{"filter":{filter},"as":"card"}}}},{{"Move":{{"what":{{"Bound":"card"}},"to":{{"Battlefield":{{}}}}}}}},{{"Shuffle":{{}}}}]}}}}"#
    ))
    .unwrap()
}

#[rstest]
#[case::zero_or_one(
    r#"[{"Type":"Artifact"},{"AnyOf":[{"GenericCost":0},{"GenericCost":1}]}]"#,
    "Search your library for an artifact card with mana cost {0} or {1}, put it onto the battlefield, then shuffle.",
    true
)]
#[case::one(
    r#"[{"Type":"Artifact"},{"GenericCost":1}]"#,
    "Search your library for an artifact card with mana cost {1}, put it onto the battlefield, then shuffle.",
    true
)]
#[case::one_is_not_zero_or_one(
    r#"[{"Type":"Artifact"},{"GenericCost":1}]"#,
    "Search your library for an artifact card with mana cost {0} or {1}, put it onto the battlefield, then shuffle.",
    false
)]
#[case::mana_value_is_not_mana_cost(
    r#"[{"Type":"Artifact"},{"Compare":{"stat":"ManaValue","op":"Le","to":1}}]"#,
    "Search your library for an artifact card with mana cost {0} or {1}, put it onto the battlefield, then shuffle.",
    false
)]
fn mana_costs_of_a_search_are_written(
    #[case] filter: &str,
    #[case] text: &str,
    #[case] written: bool,
) {
    let found = written_variants(&search(filter))
        .iter()
        .any(|variant| same_written_text(variant, text));
    assert_eq!(found, written, "{text}");
}

#[test]
fn a_quote_inside_a_granted_ability_is_written_in_single_quotes() {
    let rules: Card = serde_json::from_str(include_str!("../../cards/urzas_saga.json")).unwrap();
    let chapter_two = &rules.faces[0].abilities[1];
    let text = "II — ~ gains \"{2}, {T}: Create a 0/0 colorless Construct artifact creature token with 'This token gets +1/+1 for each artifact you control.'\"";
    assert!(
        written_variants(chapter_two)
            .iter()
            .any(|variant| same_written_text(variant, text))
    );
}

#[test]
fn the_third_chapter_of_urzas_saga_compiles_to_its_rules() {
    let printed = common::printed()
        .into_iter()
        .find(|card| card.name == "Urza's Saga")
        .unwrap();
    let compiled = compile(&printed).card.unwrap();
    let rules: Card = serde_json::from_str(include_str!("../../cards/urzas_saga.json")).unwrap();
    assert_eq!(
        compiled.faces[0].abilities.get(2),
        rules.faces[0].abilities.get(2)
    );
}
