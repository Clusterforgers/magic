//! Goal: every card the r60-03 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another zone, a limit or rider dropped, another cost) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r6003_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r6003_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r6003_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r6003_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R6003_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r6003_probe() {
    if let Ok(json) = std::env::var("R6003_JSON") {
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
    if let Ok(list) = std::env::var("R6003_COMPILE") {
        for name in list.split('|') {
            let card = common::printed()
                .into_iter()
                .find(|card| card.name == name)
                .unwrap();
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
    let Ok(list) = std::env::var("R6003_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r6003_report(name, &r6003_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r6003_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Protean Hulk")]
#[case("Moorland Rescuer")]
#[case("Sorcerous Spyglass")]
#[case("Anointed Peacekeeper")]
#[case("Dragon's Fire")]
#[case("Michelangelo's Technique")]
fn r6003_reads_back_whole(#[case] name: &str) {
    let report = r6003_report(name, &r6003_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::hulk_power("Protean Hulk", r#""stat": "ManaValue""#, r#""stat": "Power""#)]
#[case::hulk_seven("Protean Hulk", r#""most": 6"#, r#""most": 7"#)]
#[case::rescuer_mana_value("Moorland Rescuer", r#""stat": "Power","#, r#""stat": "ManaValue","#)]
#[case::rescuer_itself_too("Moorland Rescuer", r#""Other","#, "")]
#[case::spyglass_lands_too("Sorcerous Spyglass", r#""CardName""#, r#""NonlandCardName""#)]
#[case::spyglass_mana_too(
    "Sorcerous Spyglass",
    r#"{ "Activate": ["ChosenName"] }"#,
    r#"{ "ActivateKind": { "kind": "Every", "sources": ["ChosenName"] } }"#
)]
#[case::peacekeeper_three("Anointed Peacekeeper", r#""{2}""#, r#""{3}""#)]
#[case::fire_four("Dragon's Fire", r#""amount": 3"#, r#""amount": 4"#)]
#[case::fire_toughness("Dragon's Fire", r#""stat": "Power""#, r#""stat": "Toughness""#)]
fn r6003_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r6003_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r6003_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
