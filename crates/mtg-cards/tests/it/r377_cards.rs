//! Goal: every card the r37-7 pack made fully automatic reads back as its printed text clause by clause, and a near miss
//! of each (another set rule, duration, count or player) does not.
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

/// Prints the clause states of the cards named in `R377_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn probe() {
    let Ok(list) = std::env::var("R377_PROBE") else {
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

/// Writes the compiled rules file of each Scryfall record in the directory `R377_COMPILE` to the directory `R377_OUT`;
/// a tool for starting a rules file from what the compiler reads.
#[test]
fn compiled() {
    let (Ok(input), Ok(output)) = (std::env::var("R377_COMPILE"), std::env::var("R377_OUT")) else {
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

/// Prints every wording the writers give the ability JSON in `R377_WRITE`; a tool for writing rules files.
#[test]
fn written() {
    let Ok(json) = std::env::var("R377_WRITE") else {
        return;
    };
    let ability: mtg_cards::ability::Ability = serde_json::from_str(&json).unwrap();
    for text in mtg_cards::compile::written_variants(&ability) {
        println!("  {text}");
    }
}

#[rstest]
#[case("V.A.T.S.")]
#[case("False Peace")]
#[case("Empty City Ruse")]
#[case("Ral Zarek, Guest Lecturer")]
#[case("Zilortha, Strength Incarnate")]
#[case("Tromokratis")]
#[case("Mathemagics")]
#[case("Altar of the Pantheon")]
#[case("Loot, the Anomaly")]
#[case("Phyrexian Unlife")]
#[case("Keen-Eared Sentry")]
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
#[case::different_names("V.A.T.S.", r#""SameToughness""#, r#""DifferentNames""#)]
#[case::same_controller("V.A.T.S.", r#""SameToughness""#, r#""SameController""#)]
#[case::exactly_one("V.A.T.S.", r#"{"max":null,"min":0}"#, r#"{"max":1,"min":1}"#)]
#[case::next_combat_only("False Peace", r#""TheirNextTurn""#, r#""UntilSkipped""#)]
#[case::this_turn("False Peace", r#""TheirNextTurn""#, r#""UntilSkippedThisTurn""#)]
#[case::opponent_for_player("False Peace", r#""what":"Player""#, r#""what":"Opponent""#)]
#[case::player_for_opponent("Empty City Ruse", r#""what":"Opponent""#, r#""what":"Player""#)]
#[case::skip_draw("Empty City Ruse", r#""SkipCombat""#, r#"{"SkipStep":"Draw"}"#)]
#[case::called_coins(
    "Ral Zarek, Guest Lecturer",
    r#"{"FlipCoin":{"heads":true,"#,
    r#"{"FlipCoin":{"#
)]
#[case::skip_on_tails(
    "Ral Zarek, Guest Lecturer",
    r#""heads":true,"win""#,
    r#""heads":true,"lose""#
)]
#[case::minus_six("Ral Zarek, Guest Lecturer", r#"{"Minus":7}"#, r#"{"Minus":6}"#)]
#[case::each_player(
    "Zilortha, Strength Incarnate",
    r#"{"LethalByPower":"You"}"#,
    r#"{"LethalByPower":"EachPlayer"}"#
)]
#[case::by_toughness(
    "Zilortha, Strength Incarnate",
    r#"{"LethalByPower":"You"}"#,
    r#"{"ToughnessAssignsDamage":"You"}"#
)]
#[case::menace("Tromokratis", r#"["BlockedByAll"]"#, r#"[{"MinBlockers":2}]"#)]
#[case::unblockable("Tromokratis", r#"["BlockedByAll"]"#, r#"["CantBeBlocked"]"#)]
#[case::x_cards("Mathemagics", r#"{"TwoToThe":"X"}"#, r#""X""#)]
#[case::you_draw("Mathemagics", r#""who":{"Target":0}"#, r#""who":"You""#)]
#[case::by_two("Altar of the Pantheon", r#""by":1"#, r#""by":2"#)]
#[case::opponents_devotion("Altar of the Pantheon", r#""who":"You""#, r#""who":"Opponents""#)]
#[case::by_toughness(
    "Loot, the Anomaly",
    r#""AssignsAsThoughPositive""#,
    r#""AssignsByToughness""#
)]
#[case::always(
    "Phyrexian Unlife",
    r#""condition":{"Compare":{"left":{"Life":"You"},"op":"Le","right":0}},"#,
    ""
)]
#[case::at_ten("Phyrexian Unlife", r#""right":0"#, r#""right":10"#)]
#[case::opponents_infected(
    "Phyrexian Unlife",
    r#"{"DamageAsInfect":"You"}"#,
    r#"{"DamageAsInfect":"Opponents"}"#
)]
#[case::twice("Keen-Eared Sentry", r#""per_turn":1"#, r#""per_turn":2"#)]
#[case::you_too("Keen-Eared Sentry", r#""who":"Opponents""#, r#""who":"EachPlayer""#)]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
