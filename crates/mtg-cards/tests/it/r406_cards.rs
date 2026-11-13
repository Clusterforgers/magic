//! Goal: every card the r40-6 pack made fully automatic reads back as its printed text clause by clause, the compiler
//! reads each replacement of a keyword action (explore, connive, proliferate) into the rules the file holds, and a near
//! miss (another count, another step, another player's creatures) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r406_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r406_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r406_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r406_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R406_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r406_probe() {
    if let Ok(json) = std::env::var("R406_JSON") {
        for line in json.split('\n') {
            let ability: Ability = serde_json::from_str(line).unwrap();
            println!("-- {line}");
            for text in mtg_cards::compile::written_variants(&ability)
                .iter()
                .take(4)
            {
                println!("  written: {text}");
            }
        }
    }
    let Ok(list) = std::env::var("R406_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r406_report(name, &r406_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r406_file(name)).unwrap();
        for ability in &card.faces[0].abilities {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Tekuthal, Inquiry Dominus")]
#[case("Topography Tracker")]
#[case("Twists and Turns // Mycoid Maze")]
#[case("Leader, Super-Genius")]
#[case("Ashes of the Abhorrent")]
fn r406_reads_back_whole(#[case] name: &str) {
    let report = r406_report(name, &r406_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::proliferate_once(
    "Tekuthal, Inquiry Dominus",
    r#"[{ "Proliferate": {} }, { "Proliferate": {} }]"#,
    r#"[{ "Proliferate": {} }]"#
)]
#[case::an_opponent_proliferates(
    "Tekuthal, Inquiry Dominus",
    r#""Proliferates": {}"#,
    r#""Proliferates": { "who": "Opponents" }"#
)]
#[case::explore_three_times(
    "Topography Tracker",
    r#"{ "Explore": { "what": "Triggering" } }
"#,
    r#"{ "Explore": { "what": "Triggering" } }, { "Explore": { "what": "Triggering" } }
"#
)]
#[case::any_creature(
    "Topography Tracker",
    r#"[{ "Type": "Creature" }, { "ControlledBy": "You" }]"#,
    r#"[{ "Type": "Creature" }]"#
)]
#[case::scry_two(
    "Twists and Turns // Mycoid Maze",
    r#"{ "Scry": {} }"#,
    r#"{ "Scry": { "count": 2 } }"#
)]
#[case::surveil_instead(
    "Twists and Turns // Mycoid Maze",
    r#"{ "Scry": {} }"#,
    r#"{ "Surveil": {} }"#
)]
#[case::draw_two(
    "Leader, Super-Genius",
    r#"{ "Draw": {} }"#,
    r#"{ "Draw": { "count": 2 } }"#
)]
#[case::an_opponents_creature(
    "Leader, Super-Genius",
    r#""Connives": { "what": [{ "Type": "Creature" }, { "ControlledBy": "You" }] }"#,
    r#""Connives": { "what": [{ "Type": "Creature" }, { "ControlledBy": "Opponents" }] }"#
)]
#[case::from_libraries(
    "Ashes of the Abhorrent",
    r#""Activate": [{ "In": "Graveyard" }]"#,
    r#""Activate": [{ "In": "Library" }]"#
)]
#[case::only_casting(
    "Ashes of the Abhorrent",
    r#""Cast": { "filter": [{ "In": "Graveyard" }] }"#,
    r#""Cast": { "filter": [{ "In": "Exile" }] }"#
)]
fn r406_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r406_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r406_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}

/// The compiler reads each replacement of a keyword action alone into the ability the card's file holds for it.
#[rstest]
#[case::proliferate_twice(
    "Tekuthal, Inquiry Dominus",
    "If you would proliferate, proliferate twice instead.",
    1
)]
#[case::explore_again(
    "Topography Tracker",
    "If a creature you control would explore, instead it explores, then it explores again.",
    1
)]
#[case::scry_first(
    "Twists and Turns // Mycoid Maze",
    "If a creature you control would explore, instead you scry 1, then that creature explores.",
    0
)]
#[case::draw_first(
    "Leader, Super-Genius",
    "If a creature you control would connive, instead you draw a card, then that creature connives.",
    0
)]
fn r406_compiler_reads_the_wording(#[case] name: &str, #[case] text: &str, #[case] ability: usize) {
    let read = mtg_cards::compile::w11d_room_clause(text, mtg_cards::compile::ClauseKind::Static);
    let file: mtg_cards::card::Card = serde_json::from_str(&r406_file(name)).unwrap();
    assert_eq!(
        read.as_ref(),
        Some(&file.faces[0].abilities[ability]),
        "{name}"
    );
}
