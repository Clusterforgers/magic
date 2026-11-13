//! Goal: every card the r55-19 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another group, another zone, another count) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r5519_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r5519_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r5519_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r5519_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R5519_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r5519_probe() {
    let Ok(list) = std::env::var("R5519_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r5519_report(name, &r5519_file(name));
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

#[rstest]
#[case("Ashiok, Dream Render")]
#[case("Leadership Vacuum")]
#[case("Tomik, Distinguished Advokist")]
#[case("Heroic Return")]
#[case("Done for the Day")]
fn r5519_reads_back_whole(#[case] name: &str) {
    let report = r5519_report(name, &r5519_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::every_players_search(
    "Ashiok, Dream Render",
    r#""OwnSpellsCantSearch": "Opponents""#,
    r#""OwnSpellsCantSearch": "EachPlayer""#
)]
#[case::to_the_graveyard("Leadership Vacuum", r#""to": "Command""#, r#""to": "Graveyard""#)]
#[case::every_creature("Leadership Vacuum", r#""Commander","#, r#"{ "Type": "Creature" },"#)]
#[case::exiled_lands(
    "Tomik, Distinguished Advokist",
    r#"{ "In": "Graveyard" }] }"#,
    r#"{ "In": "Exile" }] }"#
)]
#[case::anyones_spells(
    "Tomik, Distinguished Advokist",
    r#""BeTargeted": [{ "ControlledBy": "Opponents" }]"#,
    r#""BeTargeted": []"#
)]
#[case::three_counters("Heroic Return", r#"["+1/+1", 2]"#, r#"["+1/+1", 3]"#)]
#[case::a_soldier("Heroic Return", r#""Subtype": "Hero""#, r#""Subtype": "Soldier""#)]
#[case::upkeep("Done for the Day", r#""step": "End""#, r#""step": "Upkeep""#)]
fn r5519_near_misses_do_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r5519_file(name);
    assert!(file.contains(from), "{name} holds {from}");
    let report = r5519_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name} with {to}");
}
