//! Goal: every card the pack158-other pack made fully automatic reads back as its printed text clause by clause, and a
//! near miss (another zone, another player, another number) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn p158_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn p158_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn p158_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &p158_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `P158_PROBE` (separated by `|`), the compiler's reading of those in
/// `P158_COMPILE` and the wordings of the abilities in `P158_JSON` (one a line); a tool for writing rules files.
#[test]
fn p158_probe() {
    if let Ok(json) = std::env::var("P158_JSON") {
        for line in json.split('\n').filter(|line| !line.trim().is_empty()) {
            let ability: Ability = serde_json::from_str(line).unwrap();
            println!("-- {line}");
            for text in mtg_cards::compile::written_variants(&ability)
                .iter()
                .take(6)
            {
                println!("  written: {text}");
            }
        }
    }
    if let Ok(list) = std::env::var("P158_COMPILE") {
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
    let Ok(list) = std::env::var("P158_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = p158_report(name, &p158_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&p158_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Reprieve")]
#[case("Firkraag, Cunning Instigator")]
#[case("Urza, Planeswalker")]
fn p158_reads_back_whole(#[case] name: &str) {
    let report = p158_report(name, &p158_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::reprieve_to_the_graveyard("Reprieve", r#""to": "Hand""#, r#""to": "Graveyard""#)]
#[case::reprieve_no_draw("Reprieve", r#", { "Draw": {} }"#, "")]
#[case::firkraag_a_dragon_of_anyone(
    "Firkraag, Cunning Instigator",
    r#"{ "Subtype": "Dragon" }, { "ControlledBy": "You" }"#,
    r#"{ "Subtype": "Dragon" }"#
)]
#[case::firkraag_no_requirement(
    "Firkraag, Cunning Instigator",
    r#""if": { "P158HadToAttack": "Dealer" },"#,
    ""
)]
#[case::urza_every_planeswalker(
    "Urza, Planeswalker",
    r#""P158LoyaltyTwiceThis""#,
    r#"{ "LoyaltyTwice": "You" }"#
)]
fn p158_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = p158_file(name);
    assert!(file.contains(from), "{name}: the file has {from}");
    let report = p158_report(name, &file.replacen(from, to, 1));
    assert_ne!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {from} -> {to}"
    );
}
