//! Goal: every card the r47-00 pack made automatic reads back as its printed text clause by clause, and a near miss of
//! each new piece does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, canonical, slug};
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

/// Prints the clause states of the cards named in `R4700_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn probe() {
    let Ok(list) = std::env::var("R4700_PROBE") else {
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

/// Prints what the compiler reads from the text `R4700_TEXT` printed on the corpus card `R4700_ON`; a tool for finding
/// the wordings the readers already know.
#[test]
fn compiled_text() {
    let (Ok(text), Ok(on)) = (std::env::var("R4700_TEXT"), std::env::var("R4700_ON")) else {
        return;
    };
    let mut card = common::printed()
        .into_iter()
        .find(|card| card.name == on)
        .unwrap();
    card.faces[0].oracle_text = text;
    let compiled = mtg_cards::compile::compile(&card);
    for clause in &compiled.clauses {
        println!("  {:?} read={} {}", clause.kind, clause.read, clause.text);
    }
    if let Some(card) = compiled.card {
        println!("{}", canonical(&card));
    }
}

/// Prints every wording the writers give the ability JSON in `R4700_WRITE`; a tool for writing rules files.
#[test]
fn written() {
    let Ok(json) = std::env::var("R4700_WRITE") else {
        return;
    };
    let ability: mtg_cards::ability::Ability = serde_json::from_str(&json).unwrap();
    for text in mtg_cards::compile::written_variants(&ability) {
        println!("  W: {text}");
    }
}

/// Prints what the compiler reads from the corpus cards named in `R4700_COMPILE` (separated by `|`); a tool for
/// writing rules files.
#[test]
fn compiled_cards() {
    let Ok(list) = std::env::var("R4700_COMPILE") else {
        return;
    };
    let cards = common::printed();
    for name in list.split('|') {
        let Some(card) = cards.iter().find(|card| card.name == name) else {
            println!("== {name}: no fixture");
            continue;
        };
        let compiled = mtg_cards::compile::compile(card);
        println!("== {name}");
        for clause in &compiled.clauses {
            println!("  {:?} read={} {}", clause.kind, clause.read, clause.text);
        }
        if std::env::var("R4700_JSON").is_ok()
            && let Some(card) = compiled.card
        {
            println!("{}", canonical(&card));
        }
    }
}

#[rstest]
#[case("Multiversal Passage")]
#[case("Seal of the Guildpact")]
#[case("Tablet of the Guilds")]
#[case("Volrath's Laboratory")]
#[case("Riptide Replicator")]
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
#[case::any_land_type("Multiversal Passage", r#""BasicLandType""#, r#""LandType""#)]
#[case::three_life(
    "Multiversal Passage",
    r#"{"PayLifeOrTapped":2}"#,
    r#"{"PayLifeOrTapped":3}"#
)]
#[case::type_in_addition("Multiversal Passage", r#"{"SetLandTypes":[]},"#, "")]
#[case::three_colors("Seal of the Guildpact", r#"{"Colors":2}"#, r#"{"Colors":3}"#)]
#[case::one_color("Seal of the Guildpact", r#"{"Colors":2}"#, r#""Color""#)]
#[case::costs_more("Seal of the Guildpact", r#""Reduce""#, r#""Increase""#)]
#[case::its_colors("Seal of the Guildpact", r#""R4700ChosenColors""#, r#""Colors""#)]
#[case::any_spell("Tablet of the Guilds", r#""by":"You""#, r#""by":"Opponents""#)]
#[case::no_condition(
    "Tablet of the Guilds",
    r#""if":{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"R4700ChosenColors"}},"op":"Ge","right":1}},"#,
    ""
)]
#[case::two_needed(
    "Tablet of the Guilds",
    r#""op":"Ge","right":1"#,
    r#""op":"Ge","right":2"#
)]
#[case::color_only("Volrath's Laboratory", r#""ColorAndCreatureType""#, r#""Color""#)]
#[case::bigger("Volrath's Laboratory", r#"{"size":2}"#, r#"{"size":3}"#)]
#[case::more_mana("Volrath's Laboratory", r#""{5}""#, r#""{4}""#)]
#[case::other_counters(
    "Riptide Replicator",
    r#"{"R4700OfChosen":{"size":{"Counters":{"counter":"charge""#,
    r#"{"R4700OfChosen":{"size":{"Counters":{"counter":"+1/+1""#
)]
#[case::two_tokens(
    "Riptide Replicator",
    r#"{"token":{"R4700OfChosen""#,
    r#"{"count":2,"token":{"R4700OfChosen""#
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
