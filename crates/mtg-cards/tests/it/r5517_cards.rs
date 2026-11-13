//! Goal: every card the r55-17 pack made automatic reads back as its printed text clause by clause, a near miss of each
//! new piece does not, and the validator refuses each new node outside the shapes the engine runs.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn report_of(name: &str, text: &str) -> mtg_cards::proposal::Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    check(text, card, &tokens, &names(&cards), &Caps::everything())
}

fn file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

/// A rules file of the corpus with one piece of its compact JSON replaced.
fn altered(name: &str, from: &str, to: &str) -> String {
    let compact: serde_json::Value = serde_json::from_str(&file(name)).unwrap();
    let compact = serde_json::to_string(&compact).unwrap();
    assert!(compact.contains(from), "{name} holds {from}");
    compact.replacen(from, to, 1)
}

const ARCAVIOS: &str = "Invasion of Arcavios // Invocation of the Founders";

#[rstest]
#[case(ARCAVIOS)]
#[case("Old Stickfingers")]
#[case("Thought Dissector")]
#[case("Rain of Gore")]
#[case("Crafty Cutpurse")]
#[case("Gather Specimens")]
fn reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::no_graveyard(ARCAVIOS, r#""also":["Graveyard","Outside"]"#, r#""also":["Outside"]"#)]
#[case::no_outside(
    ARCAVIOS,
    r#""also":["Graveyard","Outside"]"#,
    r#""also":["Graveyard"]"#
)]
#[case::always_shuffled(
    ARCAVIOS,
    r#"{"If":{"condition":{"Searched":{"zone":"Library"}},"then":[{"Shuffle":{}}]}}"#,
    r#"{"Shuffle":{}}"#
)]
#[case::one_creature("Old Stickfingers", r#""count":"X","#, "")]
#[case::creatures_to_hand(
    "Old Stickfingers",
    r#"{"Move":{"to":"Graveyard","what":{"Bound":"found"}}}"#,
    r#"{"Move":{"to":"Hand","what":{"Bound":"found"}}}"#
)]
#[case::no_most("Thought Dissector", r#""most":"X","#, "")]
#[case::kept_dissector("Thought Dissector", r#",{"Sacrifice":{"filter":["This"]}}"#, "")]
#[case::any_life_gain("Rain of Gore", r#""caused_own":true,"#, "")]
#[case::only_opponents("Rain of Gore", r#""EachPlayer""#, r#""Opponents""#)]
#[case::your_own_tokens(
    "Crafty Cutpurse",
    r#""controller":"Opponents""#,
    r#""controller":"EachPlayer""#
)]
#[case::forever("Crafty Cutpurse", r#""UntilEndOfTurn""#, r#""Forever""#)]
#[case::no_tokens(
    "Gather Specimens",
    r#",{"Shield":{"by":"UnderYourControl","duration":"UntilEndOfTurn","when":{"TokenCreation":{"controller":"Opponents","what":[{"Type":"Creature"}]}}}}"#,
    ""
)]
#[case::any_token(
    "Gather Specimens",
    r#""controller":"Opponents","what":[{"Type":"Creature"}]"#,
    r#""controller":"Opponents""#
)]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {to}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::outside_first(
    ARCAVIOS,
    r#""also":["Graveyard","Outside"]"#,
    r#""zone":"Outside","also":["Graveyard","Library"]"#
)]
#[case::outside_of_another_player(
    ARCAVIOS,
    r#""also":["Graveyard","Outside"]"#,
    r#""of":"You","also":["Graveyard","Outside"]"#
)]
#[case::outside_twice(
    ARCAVIOS,
    r#""also":["Graveyard","Outside"]"#,
    r#""also":["Outside","Outside"]"#
)]
#[case::under_you_for_draws(
    "Crafty Cutpurse",
    r#"{"TokenCreation":{"controller":"Opponents"}}"#,
    r#"{"Draw":{}}"#
)]
#[case::under_you_for_your_tokens(
    "Crafty Cutpurse",
    r#"{"TokenCreation":{"controller":"Opponents"}}"#,
    r#"{"TokenCreation":{}}"#
)]
#[case::most_of_nothing("Thought Dissector", r#""most":"X""#, r#""most":"Nothing""#)]
fn the_validator_refuses_other_shapes(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_eq!(
        report.verdict(),
        Verdict::Invalid,
        "{name}: {to}: {:#?}",
        report.problems
    );
}
