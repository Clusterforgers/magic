//! Goal: every card the l32-a pack made fully automatic (payments any number of times, attacks reselected, abilities
//! that resolve with an illegal target) reads back as its printed text clause by clause, and a near miss does not.
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

/// Prints the clause states of the cards named in `L32A_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn probe() {
    let Ok(list) = std::env::var("L32A_PROBE") else {
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

/// Prints what the compiler reads from the text `L32A_TEXT` printed on the corpus card `L32A_ON`; a tool for finding
/// the wordings the readers already know.
#[test]
fn compiled_text() {
    let (Ok(text), Ok(on)) = (std::env::var("L32A_TEXT"), std::env::var("L32A_ON")) else {
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
#[case("Intrepid Adversary")]
#[case("Tainted Adversary")]
#[case("Gilded Drake")]
#[case("Portal Mage")]
#[case("Windshaper Planetar")]
#[case("Misleading Signpost")]
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
#[case::another_cost("Intrepid Adversary", r#""cost":"{1}{W}""#, r#""cost":"{2}{W}""#)]
#[case::paid_once(
    "Intrepid Adversary",
    r#"{"PayAnyTimes":{"cost":"{1}{W}"}}"#,
    r#"{"Pay":{"cost":"{1}{W}"}}"#
)]
#[case::opponent_pays(
    "Intrepid Adversary",
    r#"{"PayAnyTimes":{"cost":"{1}{W}"}}"#,
    r#"{"PayAnyTimes":{"who":"Opponents","cost":"{1}{W}"}}"#
)]
#[case::one_counter("Intrepid Adversary", r#""count":"Triggering","#, "")]
#[case::that_many_zombies(
    "Tainted Adversary",
    r#"{"Times":{"by":2,"of":"Triggering"}}"#,
    r#""Triggering""#
)]
#[case::thrice("Tainted Adversary", r#""by":2"#, r#""by":3"#)]
#[case::fizzles("Gilded Drake", r#","StillResolves""#, "")]
#[case::not_sacrificed("Gilded Drake", r#"{"Not":"Performed"}"#, r#""Performed""#)]
#[case::exactly_one("Gilded Drake", r#""bounds":{"max":1,"min":0},"#, "")]
#[case::any_time(
    "Portal Mage",
    r#","if":{"AtTrigger":{"Step":"DeclareAttackers"}}"#,
    ""
)]
#[case::players_only(
    "Portal Mage",
    r#"{"Reattack":{"what":{"Target":0}}}"#,
    r#"{"Reattack":{"what":{"Target":0},"players":true}}"#
)]
#[case::blockers_step(
    "Windshaper Planetar",
    r#""Step":"DeclareAttackers""#,
    r#""Step":"DeclareBlockers""#
)]
#[case::own_attackers(
    "Windshaper Planetar",
    r#"{"Each":[{"Type":"Creature"},"Attacking"]}"#,
    r#"{"Each":[{"Type":"Creature"},"Attacking",{"ControlledBy":"You"}]}"#
)]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
