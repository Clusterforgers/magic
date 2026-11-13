//! Goal: every card the r47-10 pack made fully automatic reads back as its printed text clause by clause, the compiler
//! reads its new wordings into the same rules, and a near miss does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r4710_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r4710_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r4710_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r4710_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R4710_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r4710_probe() {
    if let Ok(json) = std::env::var("R4710_JSON") {
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
    if let Ok(list) = std::env::var("R4710_COMPILE") {
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
    let Ok(list) = std::env::var("R4710_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r4710_report(name, &r4710_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r4710_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Jared Carthalion, True Heir")]
#[case("Kangee, Aerie Keeper")]
#[case("Matter Reshaper")]
#[case("Gisa's Favorite Shovel")]
#[case("Imperial Mask")]
fn r4710_reads_back_whole(#[case] name: &str) {
    let report = r4710_report(name, &r4710_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::opponents_kept_out(
    "Jared Carthalion, True Heir",
    r#""Cant": { "who": "You""#,
    r#""Cant": { "who": "Opponents""#
)]
#[case::you_become_it(
    "Jared Carthalion, True Heir",
    r#""BecomeMonarch": { "who": { "Target": 0 } }"#,
    r#""BecomeMonarch": {}"#
)]
#[case::until_your_next_turn(
    "Jared Carthalion, True Heir",
    r#""duration": "UntilEndOfTurn""#,
    r#""duration": "UntilYourNextTurn""#
)]
#[case::plus_one_counters(
    "Kangee, Aerie Keeper",
    r#""counter": { "Other": "feather" }, "count": "X""#,
    r#""counter": "+1/+1", "count": "X""#
)]
#[case::one_feather("Kangee, Aerie Keeper", r#""count": "X""#, r#""count": 1"#)]
#[case::mana_value_two("Matter Reshaper", r#""Le", "to": 3"#, r#""Le", "to": 2"#)]
#[case::otherwise_milled("Matter Reshaper", r#""to": "Hand""#, r#""to": "Graveyard""#)]
#[case::each_opponent_sacrifices(
    "Gisa's Favorite Shovel",
    r#""who": "Defending""#,
    r#""who": "Opponents""#
)]
#[case::a_token_regardless(
    "Gisa's Favorite Shovel",
    r#""op": "Ge", "right": 1"#,
    r#""op": "Ge", "right": 0"#
)]
#[case::each_opponent_copies("Imperial Mask", r#""who": "Teammates""#, r#""who": "Opponents""#)]
#[case::tokens_copy_too(
    "Imperial Mask",
    r#""if": { "Not": { "Matches": { "subject": "This", "selector": ["Token"] } } },"#,
    ""
)]
fn r4710_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r4710_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r4710_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
