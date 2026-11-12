//! Goal: every card the l31-d pack made fully automatic reads back as its printed text clause by clause, and each card
//! it left partial keeps by hand only the clauses it names.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, canonical, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use mtg_import::scryfall::cards;
use rstest::rstest;

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn report(name: &str) -> mtg_cards::proposal::Report {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    report_of(name, &std::fs::read_to_string(&path).unwrap())
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

/// Prints the clause states of the cards named in `L31D_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn probe() {
    let Ok(list) = std::env::var("L31D_PROBE") else {
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

/// Writes the compiled rules file of each Scryfall record in the directory `L31D_COMPILE` to the directory `L31D_OUT`;
/// a tool for starting a rules file from what the compiler reads.
#[test]
fn compiled() {
    let (Ok(input), Ok(output)) = (std::env::var("L31D_COMPILE"), std::env::var("L31D_OUT")) else {
        return;
    };
    for entry in std::fs::read_dir(&input).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let Some(Ok(record)) = cards(format!("[{text}]").as_bytes()).next() else {
            continue;
        };
        let printed = common::printed_of(&record);
        let Some(card) = mtg_cards::compile::compile(&printed).card else {
            continue;
        };
        let out = std::path::Path::new(&output).join(path.file_name().unwrap());
        std::fs::write(out, canonical(&card)).unwrap();
    }
}

/// Prints every wording the writers give the ability JSON in `L31D_WRITE`; a tool for writing rules files.
#[test]
fn written() {
    let Ok(json) = std::env::var("L31D_WRITE") else {
        return;
    };
    let ability: mtg_cards::ability::Ability = serde_json::from_str(&json).unwrap();
    for text in mtg_cards::compile::written_variants(&ability) {
        println!("  {text}");
    }
}

#[rstest]
#[case("Crypt Rats")]
#[case("Crimson Hellkite")]
#[case("Consume Spirit")]
#[case("Feasting Hobbit")]
#[case("God-Eternal Bontu")]
#[case("Illuminate History")]
#[case("Landslide")]
#[case("Last-Ditch Effort")]
#[case("Mana Seism")]
#[case("Pitiless Carnage")]
#[case("Preyseizer Dragon")]
#[case("Renounce")]
#[case("Reprocess")]
#[case("Ribtruss Roaster")]
#[case("Sacred Rites")]
#[case("The Elder Dragon War")]
fn reads_back_whole(#[case] name: &str) {
    let report = report(name);
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

/// A rules file of the corpus with one piece of its JSON replaced.
fn altered(name: &str, from: &str, to: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    let text = std::fs::read_to_string(&path).unwrap();
    let compact: serde_json::Value = serde_json::from_str(&text).unwrap();
    let compact = serde_json::to_string(&compact).unwrap();
    assert!(compact.contains(from), "{name} holds {from}");
    compact.replacen(from, to, 1)
}

#[rstest]
#[case::red_rats("Crypt Rats", r#"{"XColor":"Black"}"#, r#"{"XColor":"Red"}"#)]
#[case::any_mana_rats("Crypt Rats", r#",{"XColor":"Black"}"#, "")]
#[case::black_hellkite("Crimson Hellkite", r#"{"XColor":"Red"}"#, r#"{"XColor":"Black"}"#)]
#[case::any_mana_hellkite("Crimson Hellkite", r#",{"XColor":"Red"}"#, "")]
#[case::white_spirit("Consume Spirit", r#"{"XColor":"Black"}"#, r#"{"XColor":"White"}"#)]
#[case::optional_spirit("Consume Spirit", r#","required":true"#, "")]
#[case::lands_for_mountains(
    "Landslide",
    r#"{"Subtype":"Mountain"},{"ControlledBy"#,
    r#"{"Type":"Land"},{"ControlledBy"#
)]
#[case::up_to_two(
    "Last-Ditch Effort",
    r#""count":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#,
    r#""count":2"#
)]
#[case::thrice("Renounce", r#""by":2"#, r#""by":3"#)]
#[case::bontu_any("God-Eternal Bontu", r#""filter":["Other"]"#, r#""filter":[]"#)]
#[case::must_discard("Illuminate History", r#","optional":true"#, "")]
#[case::seism_another_count(
    "Mana Seism",
    r#""times":{"Bound":"sacrificed~moved"}"#,
    r#""times":2"#
)]
#[case::carnage_opponents(
    "Pitiless Carnage",
    r#"{"SacrificeUpTo":{"#,
    r#"{"SacrificeUpTo":{"who":"Opponents","#
)]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
