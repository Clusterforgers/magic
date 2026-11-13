//! Goal: every card the r47-09 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another count, another quality, another player) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r4709_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r4709_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r4709_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    check(
        text,
        card,
        &tokens,
        &r4709_names(&cards),
        &Caps::everything(),
    )
}

/// The printed card of the corpus named `name`, else of the Scryfall object at `$R4709_FIXTURES/<slug>.json`.
fn r4709_printed_named(name: &str) -> PrintedCard {
    if let Some(card) = common::printed().into_iter().find(|card| card.name == name) {
        return card;
    }
    let dir = std::env::var("R4709_FIXTURES").unwrap();
    let text = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap())
        .find(|text| text.contains(&format!("\"name\":\"{name}\"")))
        .unwrap();
    let record = mtg_import::scryfall::cards(format!("[{text}]").as_bytes())
        .next()
        .unwrap()
        .unwrap();
    common::printed_of(&record)
}

/// Prints the clause states of the cards named in `R4709_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r4709_probe() {
    if let Ok(json) = std::env::var("R4709_JSON") {
        for line in json.split('\n') {
            let ability: Ability = serde_json::from_str(line).unwrap();
            println!("-- {line}");
            for text in mtg_cards::compile::written_variants(&ability)
                .iter()
                .take(4)
            {
                println!("  written: {text}");
            }
        }
    }
    if let Ok(list) = std::env::var("R4709_COMPILE") {
        for name in list.split('|') {
            let card = r4709_printed_named(name);
            let compiled = mtg_cards::compile::compile(&card);
            println!("== {name}");
            for clause in &compiled.clauses {
                println!("  clause read={} {}", clause.read, clause.text);
            }
            if let Some(card) = compiled.card {
                for face in &card.faces {
                    for ability in &face.abilities {
                        println!("  {}", serde_json::to_string(ability).unwrap());
                    }
                }
            }
        }
    }
    let Ok(list) = std::env::var("R4709_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r4709_report(name, &r4709_file(name));
        println!("== {name}: {:?}", report.verdict());
        for problem in &report.problems {
            println!("  problem: {problem:?}");
        }
        for clause in &report.clauses {
            println!("  [{}] {} => {:?}", clause.face, clause.text, clause.state);
        }
        for stray in &report.strays {
            println!("  stray: {stray:?}");
        }
        let card: mtg_cards::card::Card = serde_json::from_str(&r4709_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Sigarda's Vanguard")]
#[case("Sigardian Zealot")]
#[case("Battle for Bretagard")]
#[case("Rip, Spawn Hunter")]
#[case("Ormos, Archive Keeper")]
#[case("Transmutation Font")]
fn r4709_reads_back_whole(#[case] name: &str) {
    let report = r4709_report(name, &r4709_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::any_creatures(
    "Sigarda's Vanguard",
    r#""as": "chosen",
                  "differ": "DifferentPowers""#,
    r#""as": "chosen""#
)]
#[case::different_names(
    "Sigarda's Vanguard",
    r#""differ": "DifferentPowers""#,
    r#""differ": "DifferentNames""#
)]
#[case::only_yours(
    "Sigarda's Vanguard",
    r#""Each": [{ "Type": "Creature" }] }"#,
    r#""Each": [{ "Type": "Creature" }, { "ControlledBy": "You" }] }"#
)]
#[case::at_least_one("Sigardian Zealot", r#""min": 0"#, r#""min": 1"#)]
#[case::any_powers(
    "Sigardian Zealot",
    r#""as": "chosen",
                  "differ": "DifferentPowers""#,
    r#""as": "chosen""#
)]
#[case::different_powers(
    "Battle for Bretagard",
    r#""differ": "DifferentNames""#,
    r#""differ": "DifferentPowers""#
)]
#[case::nontoken("Battle for Bretagard", r#""Token","#, "")]
#[case::mana_values(
    "Rip, Spawn Hunter",
    r#""differ": "DifferentPowers""#,
    r#""differ": "DifferentManaValues""#
)]
#[case::any_powers_kept(
    "Rip, Spawn Hunter",
    r#""as": "kept",
                  "differ": "DifferentPowers""#,
    r#""as": "kept""#
)]
#[case::any_names(
    "Ormos, Archive Keeper",
    r#""filter": [{ "Apart": "DifferentNames" }]"#,
    r#""filter": []"#
)]
#[case::other_powers(
    "Ormos, Archive Keeper",
    r#""Apart": "DifferentNames""#,
    r#""Apart": "DifferentPowers""#
)]
#[case::two_cards("Ormos, Archive Keeper", r#""count": 3"#, r#""count": 2"#)]
#[case::tokens_alike("Transmutation Font", r#", { "Apart": "DifferentNames" }"#, "")]
#[case::any_artifacts("Transmutation Font", r#""Token", "#, "")]
fn r4709_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r4709_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r4709_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
