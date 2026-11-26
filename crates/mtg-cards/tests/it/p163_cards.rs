//! Goal: every card the p163 pack made fully automatic reads back as its printed text clause by clause through
//! `check_exact`, and a near miss does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Report, Verdict, check_exact};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn p163_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn p163_report(name: &str, text: &str) -> Report {
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

fn p163_verdict(name: &str, text: &str) -> Verdict {
    p163_report(name, text).verdict()
}

#[rstest]
#[case("Cyclopean Tomb")]
#[case("Soul Echo")]
#[case("Tahngarth, First Mate")]
fn p163_reads_back_whole(#[case] name: &str) {
    let report = p163_report(name, &p163_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?} {:#?}",
        report.problems,
        report.clauses
    );
}

#[rstest]
#[case::tahngarth_untapped("Tahngarth, First Mate", r#"["Tapped"]"#, r#"["Untapped"]"#)]
#[case::tahngarth_for_the_turn(
    "Tahngarth, First Mate",
    r#""UntilEndOfCombat""#,
    r#""UntilEndOfTurn""#
)]
#[case::tahngarth_any_attack(
    "Tahngarth, First Mate",
    r#""R5084JoinTheirAttack""#,
    r#""JoinAttack""#
)]
#[case::echo_charge("Soul Echo", r#"{ "Other": "echo" }, "count""#, r#""charge", "count""#)]
#[case::echo_for_the_turn("Soul Echo", r#""UntilYourNextUpkeep""#, r#""UntilEndOfTurn""#)]
#[case::echo_you_choose("Soul Echo", r#""who": { "Target": 0 },"#, "")]
#[case::tomb_for_a_turn("Cyclopean Tomb", r#""until": "Forever","#, "")]
#[case::tomb_island("Cyclopean Tomb", r#"["Swamp"]"#, r#"["Island"]"#)]
fn p163_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = p163_file(name);
    assert!(file.contains(from), "{name}: the file has {from}");
    assert_ne!(
        p163_verdict(name, &file.replacen(from, to, 1)),
        Verdict::Accepted,
        "{name}: {from} -> {to}"
    );
}
