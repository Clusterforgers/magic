//! Goal: every card the p166 pack made fully automatic reads back as its printed text clause by clause through
//! `check_exact`, and a near miss does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Report, Verdict, check_exact};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn p166_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn p166_report(name: &str, text: &str) -> Report {
    let cards = common::printed();
    let names: BTreeSet<String> = cards
        .iter()
        .flat_map(|card: &PrintedCard| {
            std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name))
        })
        .map(|name| name.to_lowercase())
        .collect();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards.iter().find(|card| card.name == name).unwrap();
    check_exact(text, card, &tokens, &names, &Caps::everything())
}

fn p166_verdict(name: &str, text: &str) -> Verdict {
    p166_report(name, text).verdict()
}

#[rstest]
#[case("Bamboozling Beeble")]
#[case("Camouflage")]
#[case("Monitor Monitor")]
fn p166_reads_back_whole(#[case] name: &str) {
    let report = p166_report(name, &p166_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?} {:#?}",
        report.problems,
        report.clauses
    );
}

#[rstest]
#[case::monitor_every_time("Monitor Monitor", r#", "once": true"#, "")]
#[case::monitor_two("Monitor Monitor", r#""cost": "{1}""#, r#""cost": "{2}""#)]
#[case::beeble_for_good("Bamboozling Beeble", r#""UntilEndOfTurn""#, r#""Forever""#)]
#[case::beeble_extra_die("Bamboozling Beeble", r#""R706IgnoreChosen""#, r#""ExtraDie""#)]
#[case::camouflage_any_step("Camouflage", r#"{ "Step": "DeclareAttackers" }, "#, "")]
#[case::camouflage_for_good("Camouflage", r#""UntilEndOfTurn""#, r#""Forever""#)]
fn p166_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = p166_file(name);
    assert!(file.contains(from), "{name}: the file has {from}");
    assert_ne!(
        p166_verdict(name, &file.replacen(from, to, 1)),
        Verdict::Accepted,
        "{name}: {from} -> {to}"
    );
}
