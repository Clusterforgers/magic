//! Goal: every card the r55-10 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another keyword, another cost read off the card, another crew limit, another host) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r5510_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r5510_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r5510_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    check(
        text,
        card,
        &tokens,
        &r5510_names(&cards),
        &Caps::everything(),
    )
}

#[rstest]
#[case("Djinn Illuminatus")]
#[case("Hatchery Sliver")]
#[case("Herigast, Erupting Nullkite")]
#[case("Aminatou, Veil Piercer")]
#[case("Zhulodok, Void Gorger")]
#[case("Henry Wu, InGen Geneticist")]
#[case("Darksteel Garrison")]
#[case("Luxurious Locomotive")]
#[case("Light Up the Night")]
fn r5510_reads_back_whole(#[case] name: &str) {
    let report = r5510_report(name, &r5510_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?} {:#?}",
        report.clauses,
        report.problems
    );
}

#[rstest]
#[case::djinn_emerge("Djinn Illuminatus", r#""Replicate""#, r#""Emerge""#)]
#[case::djinn_value("Djinn Illuminatus", r#""OwnManaCost""#, r#""OwnManaValue""#)]
#[case::sliver_any_spell("Hatchery Sliver", r#"{ "Subtype": "Sliver" },"#, "")]
#[case::herigast_replicate(
    "Herigast, Erupting Nullkite",
    r#"{ "Emerge": ["OwnManaCost"] }"#,
    r#"{ "Replicate": ["OwnManaCost"] }"#
)]
#[case::aminatou_less_two(
    "Aminatou, Veil Piercer",
    r#""OwnManaCostLess": 4"#,
    r#""OwnManaCostLess": 2"#
)]
#[case::aminatou_foretell("Aminatou, Veil Piercer", r#""Miracle""#, r#""Foretell""#)]
#[case::zhulodok_six("Zhulodok, Void Gorger", r#""to": 7"#, r#""to": 6"#)]
#[case::henry_nontoken(
    "Henry Wu, InGen Geneticist",
    r#""ExploitedNonHuman""#,
    r#""ExploitedNontoken""#
)]
#[case::henry_power_two("Henry Wu, InGen Geneticist", r#""right": 3"#, r#""right": 2"#)]
#[case::garrison_hexproof("Darksteel Garrison", r#""Indestructible""#, r#""Hexproof""#)]
#[case::garrison_equip("Darksteel Garrison", r#""Fortify""#, r#""Equip""#)]
#[case::locomotive_plain_crew("Luxurious Locomotive", r#""CrewOnce""#, r#""Crew""#)]
#[case::night_flashback_zero("Light Up the Night", r#"{ "XAtLeast": 1 }"#, r#"{ "XAtLeast": 2 }"#)]
fn r5510_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r5510_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r5510_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
