//! Goal: every card the r28-6 pack made fully automatic reads back as its printed text clause by clause, and each card
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

/// Prints the clause states of the cards named in `R286_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn probe() {
    let Ok(list) = std::env::var("R286_PROBE") else {
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

/// Writes the compiled rules file of each Scryfall record in the directory `R286_COMPILE` to the directory `R286_OUT`;
/// a tool for starting a rules file from what the compiler reads.
#[test]
fn compiled() {
    let (Ok(input), Ok(output)) = (std::env::var("R286_COMPILE"), std::env::var("R286_OUT")) else {
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

#[rstest]
#[case("Big Winner")]
#[case("Croakid Amphibonaut")]
#[case("Sanguine Sipper")]
#[case("Scared Stiff")]
#[case("Grabby Tabby")]
#[case("Anavolver")]
#[case("Cetavolver")]
#[case("Degavolver")]
#[case("Necravolver")]
#[case("Rakavolver")]
#[case("Ugin's Conjurant")]
#[case("Angel of Suffering")]
#[case("Walker of the Wastes")]
#[case("Wastes")]
#[case("Jumbo Cactuar")]
#[case("Grim Wanderer")]
#[case("Khorvath Brightflame")]
#[case("Sylvia Brightspear")]
#[case("Gas Guzzler")]
#[case("Perilous Snare")]
#[case("Pride of the Road")]
#[case("Rakdos, Lord of Riots")]
#[case("Protean Hydra")]
#[case("Polukranos, Unchained")]
#[case("Heliod, the Radiant Dawn // Heliod, the Warped Eclipse")]
#[case("Enduring Renewal")]
#[case("Altar of the Wretched // Wretched Bonemass")]
#[case("Jared Carthalion, True Heir")]
#[case("Far Fortune, End Boss")]
#[case("Okk")]
#[case("Orcish Conscripts")]
#[case("Scarred Puma")]
#[case("Master of Cruelties")]
#[case("Port Razer")]
#[case("Bloodthirster")]
#[case("Dire Wolves")]
#[case("Fortified Area")]
#[case("Mountain Stronghold")]
#[case("Cathedral of Serra")]
#[case("Wall of Shadows")]
#[case("Epochrasite")]
#[case("Viashino Bey")]
#[case("Reverent Silence")]
#[case("Skyshroud Cutter")]
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
#[case::other_kicker("Anavolver", r#"{"KickedWith":"{1}{U}"}"#, r#"{"KickedWith":"{B}"}"#)]
#[case::other_keyword(
    "Cetavolver",
    r#"{"AddKeyword":"FirstStrike"}"#,
    r#"{"AddKeyword":"Haste"}"#
)]
#[case::other_quote("Degavolver", r#"{"PayLife":3}"#, r#"{"PayLife":2}"#)]
#[case::thrice("Angel of Suffering", r#""by":2"#, r#""by":3"#)]
#[case::put_not_removed(
    "Protean Hydra",
    r#"{"RemoveCounters":{"count":"Triggering","counter":"+1/+1","from":"This"}}"#,
    r#"{"PutCounters":{"count":"Triggering","counter":"+1/+1","on":"This"}}"#
)]
#[case::other_condition(
    "Ugin's Conjurant",
    r#"{"Compare":{"left":{"Counters":{"counter":"+1/+1","on":"This"}},"op":"Ge","right":1}}"#,
    r#"{"Monarch":"You"}"#
)]
#[case::you_not_opponents(
    "Rakdos, Lord of Riots",
    r#"{"Tallied":{"what":"LifeLost","who":"Opponents"}}"#,
    r#"{"Tallied":{"what":"LifeLost","who":"You"}}"#
)]
#[case::other_name(
    "Walker of the Wastes",
    r#"{"Named":"Wastes"}"#,
    r#"{"Named":"Sylvia Brightspear"}"#
)]
#[case::not_stickered("Big Winner", r#""Stickered""#, r#""Tapped""#)]
#[case::three_others(
    "Orcish Conscripts",
    r#"{"AttackWithOthers":{"others":2}}"#,
    r#"{"AttackWithOthers":{"others":3}}"#
)]
#[case::white_or_green("Scarred Puma", r#"{"Color":"Black"}"#, r#"{"Color":"White"}"#)]
#[case::not_alone("Master of Cruelties", r#""AttackAlone""#, r#""CantAttackAlone""#)]
#[case::other_life(
    "Skyshroud Cutter",
    r#"{"OthersGainLife":5}"#,
    r#"{"OthersGainLife":6}"#
)]
#[case::bey_alone("Viashino Bey", r#""AttackWithAll""#, r#""AttackAlone""#)]
#[case::blocked_not_attacked("Port Razer", r#""event":"Attacked""#, r#""event":"Blocked""#)]
#[case::blue_legends("Mountain Stronghold", r#"{"Color":"Red"}"#, r#"{"Color":"Blue"}"#)]
#[case::from_graveyard("Epochrasite", r#""from":"Hand""#, r#""from":"Graveyard""#)]
#[case::blocking_it("Wall of Shadows", r#"{"BlockedBy":"This"}"#, r#"{"Blocks":"This"}"#)]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
