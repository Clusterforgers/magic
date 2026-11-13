//! Goal: every card the r55-16 pack made automatic (or improved) reads back as its printed text clause by clause, a near
//! miss of each new piece does not, and the validator refuses each new node outside the shapes the engine runs.
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

#[rstest]
#[case("Ajani, Nacatl Pariah // Ajani, Nacatl Avenger")]
#[case("Ancient Adamantoise")]
#[case("Cemetery Desecrator")]
#[case("Retriever Phoenix")]
#[case("Twinning Staff")]
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
#[case::no_intervening_if(
    "Ajani, Nacatl Pariah // Ajani, Nacatl Avenger",
    r#","if":{"Compare":{"left":{"Count":[{"Color":"Red"},{"ControlledBy":"You"},"Other"]},"op":"Ge","right":1}}"#,
    ""
)]
#[case::a_green_permanent(
    "Ajani, Nacatl Pariah // Ajani, Nacatl Avenger",
    r#"{"Color":"Red"},{"ControlledBy":"You"},"Other""#,
    r#"{"Color":"Green"},{"ControlledBy":"You"},"Other""#
)]
#[case::the_exiled_cards_power("Cemetery Desecrator", r#""stat":"ManaValue""#, r#""stat":"Power""#)]
#[case::exile_this_card_too(
    "Cemetery Desecrator",
    r#"{"In":"Graveyard"},"Other""#,
    r#"{"In":"Graveyard"}"#
)]
#[case::only_your_damage(
    "Ancient Adamantoise",
    r#"{"Union":["You",{"Each":[{"ControlledBy":"You"},"Other"]}]}"#,
    r#""You""#
)]
#[case::its_own_damage_too(
    "Ancient Adamantoise",
    r#"{"ControlledBy":"You"},"Other""#,
    r#"{"ControlledBy":"You"}"#
)]
#[case::another_rule("Ancient Adamantoise", r#""KeepsDamage""#, r#""ManaBurn""#)]
#[case::an_opponents_learn(
    "Retriever Phoenix",
    r#"{"Learns":{}}"#,
    r#"{"Learns":{"who":"Opponents"}}"#
)]
#[case::two_more_copies("Twinning Staff", r#"{"AddAmount":1}"#, r#"{"AddAmount":2}"#)]
#[case::an_opponents_copy(
    "Twinning Staff",
    r#"{"CopiesSpell":{}}"#,
    r#"{"CopiesSpell":{"who":"Opponents"}}"#
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
#[case::copies_replaced_by_steps(
    "Twinning Staff",
    r#"{"AddAmount":1}"#,
    r#"{"Instead":[{"Draw":{}}]}"#
)]
#[case::a_learn_replaced_by_a_draw(
    "Retriever Phoenix",
    r#"[{"Move":{"what":"This","to":{"Battlefield":{}}}}]"#,
    r#"[{"Draw":{}}]"#
)]
#[case::steps_without_a_learn("Retriever Phoenix", r#"{"Learns":{}}"#, r#"{"Draw":{}}"#)]
#[case::no_more_copies("Twinning Staff", r#"{"AddAmount":1}"#, r#"{"AddAmount":0}"#)]
#[case::an_unbound_name_in_the_if(
    "Ajani, Nacatl Pariah // Ajani, Nacatl Avenger",
    r#"{"Count":[{"Color":"Red"},{"ControlledBy":"You"},"Other"]}"#,
    r#"{"Stat":{"of":{"Bound":"nobody"},"stat":"Power"}}"#
)]
fn the_validator_refuses_other_shapes(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_eq!(
        report.verdict(),
        Verdict::Invalid,
        "{name}: {to}: {:#?}",
        report.problems
    );
}
