//! Goal: every card the r30-1b pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss of each does not.
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

/// Prints the clause states of the cards named in `R301B_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn probe() {
    let Ok(list) = std::env::var("R301B_PROBE") else {
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

/// Writes the compiled rules file of each Scryfall record in the directory `R301B_COMPILE` to the directory `R301B_OUT`;
/// a tool for starting a rules file from what the compiler reads.
#[test]
fn compiled() {
    let (Ok(input), Ok(output)) = (std::env::var("R301B_COMPILE"), std::env::var("R301B_OUT"))
    else {
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
#[case("Chandra, Fire of Kaladesh // Chandra, Roaring Flame")]
#[case("Brotherhood Headquarters")]
#[case("Fortified Area")]
#[case("Helm of Possession")]
#[case("Hivis of the Scale")]
#[case("Jasmine Boreal of the Seven")]
#[case("Keldon Battlewagon")]
#[case("Metalworker")]
#[case("Rasputin, the Oneiromancer")]
#[case("Rosheen, Roaring Prophet")]
#[case("Rubinia Soulsinger")]
#[case("Sacellum Godspeaker")]
#[case("Seasinger")]
#[case("Shelkin Brownie")]
#[case("Tolaria")]
#[case("Tree of Redemption")]
#[case("Urza's Engine")]
#[case("Vazi, Keen Negotiator")]
#[case("Vexing Puzzlebox")]
#[case("Vodalian Mystic")]
#[case("Willow Satyr")]
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
#[case::only_while_controlled(
    "Willow Satyr",
    r#""WhileYouControlSourceTapped""#,
    r#""WhileYouControlSource""#
)]
#[case::only_while_tapped(
    "Rubinia Soulsinger",
    r#""WhileYouControlSourceTapped""#,
    r#""WhileSourceTapped""#
)]
#[case::another_type("Hivis of the Scale", r#""Dragon""#, r#""Wizard""#)]
#[case::toughness("Keldon Battlewagon", r#""stat":"Power""#, r#""stat":"Toughness""#)]
#[case::fewer_counters("Vexing Puzzlebox", r#""count":100"#, r#""count":99"#)]
#[case::for_the_turn("Vodalian Mystic", r#""Forever""#, r#""UntilEndOfTurn""#)]
#[case::another_step("Tolaria", r#""Upkeep""#, r#""Draw""#)]
#[case::banding_kept("Tolaria", r#"{"RemoveKeyword":"Banding"},"#, "")]
#[case::landwalk("Shelkin Brownie", r#""BandsWithOther""#, r#""Landwalk""#)]
#[case::with_flashback("Jasmine Boreal of the Seven", r#""NoAbilities""#, r#""Flashback""#)]
#[case::spells_with_flashback("Brotherhood Headquarters", r#""Freerunning""#, r#""Flashback""#)]
#[case::any_ability(
    "Brotherhood Headquarters",
    r#",{"Ability":{"subtypes":["Assassin"]}}"#,
    r#",{"Ability":{}}"#
)]
#[case::blocking_too("Urza's Engine", r#""Attacking","#, "")]
#[case::creature_cards("Metalworker", r#""Artifact""#, r#""Creature""#)]
#[case::one_mana("Metalworker", r#"["Colorless","Colorless"]"#, r#"["Colorless"]"#)]
#[case::power_four("Sacellum Godspeaker", r#""to":5"#, r#""to":4"#)]
#[case::at_least_one("Sacellum Godspeaker", r#""min":0"#, r#""min":1"#)]
#[case::two_or_more("Rasputin, the Oneiromancer", r#"{"XAtLeast":1}"#, r#"{"XAtLeast":2}"#)]
#[case::mill_five("Rosheen, Roaring Prophet", r#""count":6"#, r#""count":5"#)]
#[case::any_cost("Rosheen, Roaring Prophet", r#","only":["CostWithX"]"#, "")]
#[case::clues(
    "Vazi, Keen Negotiator",
    r#"{"Subtype":"Treasure"}"#,
    r#"{"Subtype":"Clue"}"#
)]
#[case::opponents_treasures("Vazi, Keen Negotiator", r#""who":"You"}"#, r#""who":"Opponents"}"#)]
#[case::another_life(
    "Tree of Redemption",
    r#"{"ExchangeLifeWithToughness":{"what":"This"}}"#,
    r#"{"ExchangeLifeWithToughness":{"who":"Opponents","what":"This"}}"#
)]
#[case::a_forest(
    "Seasinger",
    r#"{"Subtype":"Island"},{"ControlledBy":"You"}]},"op":"Ge","right":1"#,
    r#"{"Subtype":"Forest"},{"ControlledBy":"You"}]},"op":"Ge","right":1"#
)]
#[case::two_islands(
    "Seasinger",
    r#"{"Subtype":"Island"},{"ControlledBy":"You"}]},"op":"Ge","right":1"#,
    r#"{"Subtype":"Island"},{"ControlledBy":"You"}]},"op":"Ge","right":2"#
)]
#[case::four_damage(
    "Chandra, Fire of Kaladesh // Chandra, Roaring Flame",
    r#""right":3"#,
    r#""right":4"#
)]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}

/// Prints the wordings of each ability of the cards named in `R301B_WORDS` (separated by `|`); a tool for writers.
#[test]
fn words() {
    let Ok(list) = std::env::var("R301B_WORDS") else {
        return;
    };
    for name in list.split('|') {
        let path = common::cards_dir().join(format!("{}.json", slug(name)));
        let card: mtg_cards::card::Card =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for face in &card.faces {
            for ability in &face.abilities {
                println!("== {name}");
                for text in mtg_cards::compile::written_variants(ability).iter().take(8) {
                    println!("  {text}");
                }
            }
        }
    }
}
