//! Goal: every card the p170 pack made fully automatic reads back as its printed text clause by clause through
//! `check_exact`, and a near miss does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Report, Verdict, check_exact};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn p170_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn p170_report(name: &str, text: &str) -> Report {
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

fn p170_verdict(name: &str, text: &str) -> Verdict {
    p170_report(name, text).verdict()
}

#[rstest]
#[case("Scotty, Miracle Worker")]
#[case("Prowl, Stoic Strategist // Prowl, Pursuit Vehicle")]
#[case("Myra the Magnificent")]
fn p170_reads_back_whole(#[case] name: &str) {
    let report = p170_report(name, &p170_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?} {:#?}",
        report.problems,
        report.clauses
    );
}

const PROWL: &str = "Prowl, Stoic Strategist // Prowl, Pursuit Vehicle";

#[rstest]
#[case::scotty_any_turn("Scotty, Miracle Worker", r#""YourTurn", "#, "")]
#[case::scotty_returns_himself("Scotty, Miracle Worker", r#", { "Not": { "Is": "Granter" } }"#, "")]
#[case::scotty_from_exile("Scotty, Miracle Worker", r#""In": "Graveyard""#, r#""In": "Exile""#)]
#[case::prowl_untapped(PROWL, r#""Other", "Tapped","#, r#""Other","#)]
#[case::prowl_you_play(PROWL, r#""who": { "OwnerOf": { "Bound": "exiled" } },"#, "")]
#[case::prowl_any_spell(
    PROWL,
    r#"{ "P170LeftExileWith": "This" }"#,
    r#"{ "Type": "Creature" }"#
)]
#[case::prowl_third_time(PROWL, r#""right": 2"#, r#""right": 3"#)]
#[case::myra_with_counter(
    "Myra the Magnificent",
    r#"{ "Not": { "HasCounter": { "Other": "midway" } } }"#,
    r#"{ "HasCounter": { "Other": "midway" } }"#
)]
#[case::myra_any_attraction(
    "Myra the Magnificent",
    r#", "what": [{ "Is": { "Bound": "attraction" } }]"#,
    ""
)]
#[case::myra_once("Myra the Magnificent", r#""once": false,"#, "")]
fn p170_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = p170_file(name);
    assert!(file.contains(from), "{name}: the file has {from}");
    assert_ne!(
        p170_verdict(name, &file.replacen(from, to, 1)),
        Verdict::Accepted,
        "{name}: {from} -> {to}"
    );
}
