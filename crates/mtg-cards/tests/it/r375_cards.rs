//! Goal: every card the r37-5 pack made fully automatic (life paid, Vehicles crewed, nontoken creatures exploited,
//! permanents taken by an opponent, land and nonland cards explored, phase-outs held) reads back as its printed text clause by clause, and a near miss does not.
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

/// Prints the clause states of the cards named in `R375_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn probe() {
    let Ok(list) = std::env::var("R375_PROBE") else {
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

/// Prints what the compiler reads from the text `R375_TEXT` printed on the corpus card `R375_ON`; a tool for finding
/// the wordings the readers already know.
#[test]
fn compiled_text() {
    let (Ok(text), Ok(on)) = (std::env::var("R375_TEXT"), std::env::var("R375_ON")) else {
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

#[rstest]
#[case("Font of Agonies")]
#[case("Veteran Motorist")]
#[case("Gearshift Ace")]
#[case("Speedway Fanatic")]
#[case("Skull Skaab")]
#[case("Zidane, Tantalus Thief")]
#[case("Nicanzil, Current Conductor")]
#[case("Teferi, Timeless Voyager")]
#[case("The Phasing of Zhalfir")]
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
#[case::opponents_pay("Font of Agonies", r#""who":"You""#, r#""who":"Opponents""#)]
#[case::one_counter("Font of Agonies", r#""count":"Triggering","#, "")]
#[case::life_lost(
    "Font of Agonies",
    r#""action":"PaidLife""#,
    r#""action":"Sacrificed""#
)]
#[case::more_power("Veteran Motorist", r#""power":1"#, r#""power":2"#)]
#[case::any_crew("Veteran Motorist", r#"{"Crewed":"Triggering"}"#, r#""Tapped""#)]
#[case::any_exploit("Skull Skaab", r#""ExploitedNontoken""#, r#""Exploited""#)]
#[case::opponents_exploit(
    "Skull Skaab",
    r#"{"ControlledBy":"You"}"#,
    r#"{"ControlledBy":"Opponents"}"#
)]
#[case::any_change(
    "Zidane, Tantalus Thief",
    r#""action":"ControlTakenByOpponent","who":"You""#,
    r#""action":"ChangedController","who":"Opponents""#
)]
#[case::from_them(
    "Zidane, Tantalus Thief",
    r#""ControlTakenByOpponent","who":"You""#,
    r#""ControlTakenByOpponent","who":"Opponents""#
)]
#[case::nonland_put(
    "Nicanzil, Current Conductor",
    r#""ExploredLand""#,
    r#""ExploredNonland""#
)]
#[case::land_counter(
    "Nicanzil, Current Conductor",
    r#""ExploredNonland""#,
    r#""ExploredLand""#
)]
#[case::any_explore("Nicanzil, Current Conductor", r#""ExploredNonland""#, r#""Explored""#)]
#[case::not_held("Teferi, Timeless Voyager", r#""held":"UntilEndOfYourNextTurn","#, "")]
#[case::held_a_turn_less(
    "Teferi, Timeless Voyager",
    r#""held":"UntilEndOfYourNextTurn""#,
    r#""held":"UntilYourNextTurn""#
)]
#[case::held_while_there(
    "The Phasing of Zhalfir",
    r#""held":"WhileYouControlSource""#,
    r#""held":"WhileSourceOnBattlefield""#
)]
#[case::zhalfir_not_held("The Phasing of Zhalfir", r#""held":"WhileYouControlSource","#, "")]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {to}: {:#?}",
        report.clauses
    );
}

/// Disciple of Caelus Nin keeps its choice of permanents by hand; its static ability reads back alone.
#[test]
fn the_disciple_reads_its_static_ability() {
    let report = report("Disciple of Caelus Nin");
    let clause = report
        .clauses
        .iter()
        .find(|clause| clause.text == "Permanents can't phase in.")
        .unwrap();
    assert!(
        matches!(clause.state, mtg_cards::proposal::ClauseState::Reads { .. }),
        "{:?}",
        clause.state
    );
}
