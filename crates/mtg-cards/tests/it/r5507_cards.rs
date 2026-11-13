//! Goal: every card the r55-07 pack made automatic reads back as its printed text clause by clause, the compiler reads
//! each new sentence it has a reader for, a near miss of each new piece does not read back, and the validator refuses
//! each new node outside the shapes the engine runs.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::compile::compile;
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
#[case("Dark Sphere")]
#[case("Forcefield")]
#[case("Rock Hydra")]
#[case("Clown Car")]
#[case("Luck Bobblehead")]
#[case("Neverwinter Hydra")]
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
#[case::rock_hydra("Rock Hydra", r#"{"PreventByCounters":"+1/+1"}"#)]
#[case::clown_car("Clown Car", r#"{"DieResult":"Odd"}"#)]
#[case::luck_bobblehead("Luck Bobblehead", r#"{"Times":6}"#)]
#[case::neverwinter_hydra("Neverwinter Hydra", r#"{"DieResult":"Total"}"#)]
fn the_compiler_reads_the_new_sentence(#[case] name: &str, #[case] piece: &str) {
    let card = common::printed()
        .into_iter()
        .find(|card| card.name == name)
        .unwrap();
    let compiled = serde_json::to_string(&compile(&card).card.unwrap()).unwrap();
    assert!(compiled.contains(piece), "{name}: {compiled}");
}

#[rstest]
#[case::half_rounded_up("Dark Sphere", r#""PreventHalfNext""#, r#""PreventHalf""#)]
#[case::all_of_it("Dark Sphere", r#""PreventHalfNext""#, r#""PreventNext""#)]
#[case::combat_damage_only("Dark Sphere", r#""to":"You"}"#, r#""to":"You","combat":true}"#)]
#[case::all_but_two(
    "Forcefield",
    r#"{"PreventAllButNext":1}"#,
    r#"{"PreventAllButNext":2}"#
)]
#[case::any_damage("Forcefield", r#""combat":true,"#, "")]
#[case::any_creature("Forcefield", r#""Unblocked","#, "")]
#[case::other_counters(
    "Rock Hydra",
    r#"{"PreventByCounters":"+1/+1"}"#,
    r#"{"PreventByCounters":"charge"}"#
)]
#[case::one_counter_for_all(
    "Rock Hydra",
    r#"{"PreventByCounters":"+1/+1"}"#,
    r#"{"PreventRemovingCounter":"+1/+1"}"#
)]
#[case::odd_for_even("Clown Car", r#"{"DieResult":"Odd"}"#, r#"{"DieResult":"Even"}"#)]
#[case::four_sided("Clown Car", r#""sides":6"#, r#""sides":4"#)]
#[case::fixed_dice("Clown Car", r#""count":"X""#, r#""count":3"#)]
#[case::untapped_treasure("Luck Bobblehead", r#","tapped":true"#, "")]
#[case::six_times("Luck Bobblehead", r#""right":7"#, r#""right":6"#)]
#[case::rolled_fives("Luck Bobblehead", r#"{"Times":6}"#, r#"{"Times":5}"#)]
#[case::odd_counters(
    "Neverwinter Hydra",
    r#"{"DieResult":"Total"}"#,
    r#"{"DieResult":"Odd"}"#
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
#[case::half_next_on_a_permanent(
    "Rock Hydra",
    r#"{"PreventByCounters":"+1/+1"}"#,
    r#""PreventHalfNext""#
)]
#[case::all_but_next_on_a_permanent(
    "Rock Hydra",
    r#"{"PreventByCounters":"+1/+1"}"#,
    r#"{"PreventAllButNext":1}"#
)]
#[case::counters_for_a_draw("Rock Hydra", r#"{"Damage":{"to":"This"}}"#, r#"{"Draw":{}}"#)]
#[case::count_with_a_table(
    "Clown Car",
    r#""results":[]"#,
    r#""results":[{"from":1,"effects":[{"Draw":{}}]}]"#
)]
#[case::count_with_a_choice(
    "Clown Car",
    r#""results":[]"#,
    r#""results":[],"dice":2,"choose":true"#
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
