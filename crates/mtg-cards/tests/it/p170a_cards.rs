//! Goal: Space Beleren reads back as its printed text clause by clause through `check_exact`, its sculptor keyword
//! included, and a near miss of any of its abilities does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Report, Verdict, check_exact};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

const P170A_CARD: &str = "Space Beleren";

fn p170a_file() -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(P170A_CARD)));
    std::fs::read_to_string(&path).unwrap()
}

fn p170a_report(text: &str) -> Report {
    let cards = common::printed();
    let names: BTreeSet<String> = cards
        .iter()
        .flat_map(|card: &PrintedCard| {
            std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name))
        })
        .map(|name| name.to_lowercase())
        .collect();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards.iter().find(|card| card.name == P170A_CARD).unwrap();
    check_exact(text, card, &tokens, &names, &Caps::everything())
}

#[test]
fn p170a_reads_back_whole() {
    let report = p170a_report(&p170a_file());
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{:#?} {:#?}",
        report.problems,
        report.clauses
    );
}

#[rstest]
#[case::another_keyword(r#""SpaceSculptor""#, r#""Banding""#)]
#[case::blocks_for_good(r#""UntilEndOfTurn""#, r#""Forever""#)]
#[case::plus_two(r#"{ "Plus": 1 }"#, r#"{ "Plus": 2 }"#)]
#[case::counters_on_all(r#", { "P170aInSector": "sector" }] },"#, "] },")]
#[case::two_counters(r#""counter": "+1/+1""#, r#""counter": "+1/+1", "count": 2"#)]
#[case::minus_four(r#"{ "Minus": 5 }"#, r#"{ "Minus": 4 }"#)]
fn p170a_a_near_miss_does_not_read_back(#[case] from: &str, #[case] to: &str) {
    let file = p170a_file();
    assert!(file.contains(from), "the file has {from}");
    let report = p170a_report(&file.replacen(from, to, 1));
    assert_ne!(
        report.verdict(),
        Verdict::Accepted,
        "{from} -> {to}: {:#?}",
        report.clauses
    );
}
