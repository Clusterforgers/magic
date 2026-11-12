//! Goal: every card the r30-7 pack made fully automatic reads back as its printed text clause by clause, and a rules
//! file with one piece changed does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{ClauseState, Report, Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn report_of(name: &str, text: &str) -> Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    check(text, card, &tokens, &names(&cards), &Caps::everything())
}

fn report(name: &str) -> Report {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    report_of(name, &std::fs::read_to_string(&path).unwrap())
}

/// Prints the clause states of each rules file in the directory `R307_DIR` checked against the corpus record of its card;
/// a tool for writing rules files.
#[test]
fn probe() {
    let Ok(dir) = std::env::var("R307_DIR") else {
        return;
    };
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    for path in paths {
        let text = std::fs::read_to_string(&path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let name = value["name"].as_str().unwrap().to_owned();
        let report = report_of(&name, &text);
        println!("== {name}: {:?}", report.verdict());
        for problem in &report.problems {
            println!("  problem: {problem:?}");
        }
        for clause in &report.clauses {
            println!("  [{}] {} => {:?}", clause.face, clause.text, clause.state);
            if let ClauseState::Unwritten { ability, .. } | ClauseState::Differs { ability, .. } =
                &clause.state
            {
                let shape = &value["faces"][clause.face]["abilities"][*ability];
                println!("WHOLE\t{shape}\t{}", clause.text);
            }
        }
        for stray in &report.strays {
            println!("  stray: {stray:?}");
        }
    }
}

#[rstest]
#[case("Captured by the Consulate")]
#[case("Charismatic Conqueror")]
#[case("Fblthp, the Lost")]
#[case("Kefka, Court Mage // Kefka, Ruler of Ruin")]
#[case("Kroxa, Titan of Death's Hunger")]
#[case("Spider-Woman, Secret Agent")]
#[case("Tiamat")]
#[case("Wrathful Raptors")]
#[case("Wrathful Red Dragon")]
#[case("Yarus, Roar of the Old Gods")]
fn reads_back_whole(#[case] name: &str) {
    let report = report(name);
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}
