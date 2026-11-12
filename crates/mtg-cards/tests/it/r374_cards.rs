//! Goal: every card the r37-4 pack made fully automatic reads back as its printed text clause by clause, and a rules
//! file with one piece changed does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
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

fn report_of(name: &str, text: &str) -> mtg_cards::proposal::Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    check(text, card, &tokens, &names(&cards), &Caps::everything())
}

fn file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    let text = std::fs::read_to_string(&path).unwrap();
    let compact: serde_json::Value = serde_json::from_str(&text).unwrap();
    serde_json::to_string(&compact).unwrap()
}

#[rstest]
#[case("Diligent Zookeeper")]
#[case("Embiggen")]
#[case("Embodiment of Agonies")]
#[case("Goliath Hatchery")]
#[case("Niv-Mizzet, Guildpact")]
#[case("Takeno, Samurai General")]
fn reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::zookeeper_cap("Diligent Zookeeper", "10]", "9]")]
#[case::embiggen_creature_types("Embiggen", r#""R374Types""#, r#""R374CreatureTypes""#)]
#[case::agonies_mana_values("Embodiment of Agonies", r#""R374ManaCost""#, r#""ManaValue""#)]
#[case::hatchery_power("Goliath Hatchery", r#""R374Toxic""#, r#""Power""#)]
#[case::niv_mizzet_any_colors(
    "Niv-Mizzet, Guildpact",
    r#"{"Compare":{"op":"Eq","stat":"Colors","to":2}}"#,
    r#"{"Compare":{"op":"Eq","stat":"Colors","to":3}}"#
)]
#[case::takeno_toxic("Takeno, Samurai General", r#""R374Bushido""#, r#""R374Toxic""#)]
fn a_changed_piece_reads_otherwise(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let text = file(name);
    assert!(text.contains(from), "{name} holds {from}");
    let report = report_of(name, &text.replacen(from, to, 1));
    assert_ne!(
        report.verdict(),
        Verdict::Accepted,
        "{name} with {to}: {:#?}",
        report.clauses
    );
}
