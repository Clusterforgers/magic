//! Goal: every card the rmt6 pack made fully automatic reads back as its printed text clause by clause, with no clause left to the table.
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

fn report(name: &str) -> mtg_cards::proposal::Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    let text = std::fs::read_to_string(&path).unwrap();
    check(&text, card, &tokens, &names(&cards), &Caps::everything())
}

/// Prints the clause states of the cards named in `RMT6_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn probe() {
    let Ok(list) = std::env::var("RMT6_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = report(name);
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
    }
}

/// Writes, for each card named in the file `RMT6_COMPILE`, what the compiler reads where the rules file leaves a clause
/// to the table; a tool for writing rules files.
#[test]
fn compiled() {
    let Ok(file) = std::env::var("RMT6_COMPILE") else {
        return;
    };
    let wanted: Vec<String> = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    for card in common::printed() {
        if !wanted.contains(&card.name) {
            continue;
        }
        let compiled = mtg_cards::compile::compile(&card);
        let Some(rules) = compiled.card else { continue };
        for (face, f) in rules.faces.iter().enumerate() {
            for ability in &f.abilities {
                if !matches!(ability, mtg_cards::ability::Ability::Manual { .. }) {
                    println!(
                        "{}\t{face}\t{}",
                        card.name,
                        serde_json::to_string(ability).unwrap()
                    );
                }
            }
        }
    }
}

#[rstest]
#[case("Fog Bank")]
#[case("Electro, Assaulting Battery")]
#[case("Doc Samson, Super Psychiatrist")]
#[case("Forethought Amulet")]
#[case("Forsaken Monument")]
#[case("Brimaz, King of Oreskos")]
#[case("Corrupted Grafstone")]
#[case("Chromatic Orrery")]
#[case("Dynaheir, Invoker Adept")]
fn reads_back_whole(#[case] name: &str) {
    let report = report(name);
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}
