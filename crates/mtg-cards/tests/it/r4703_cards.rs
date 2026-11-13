//! Goal: every card the r47-03 pack made fully automatic reads back as its printed text clause by clause, and a near miss
//! of each new wording (another color, another count, another player, another turn) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r4703_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r4703_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r4703_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r4703_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states, the written variants and the compiler's reading of the cards named in `R4703_PROBE`
/// (separated by `|`); a tool for writing rules files.
#[test]
fn r4703_probe() {
    if let Ok(json) = std::env::var("R4703_JSON") {
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
    let Ok(list) = std::env::var("R4703_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r4703_report(name, &r4703_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r4703_file(name)).unwrap();
        for face in &card.faces {
            for ability in &face.abilities {
                for text in mtg_cards::compile::written_variants(ability).iter().take(3) {
                    println!("  written: {text}");
                }
            }
        }
        let printed = common::printed()
            .into_iter()
            .find(|card| card.name == name)
            .unwrap();
        let compiled = mtg_cards::compile::compile(&printed);
        if let Some(card) = compiled.card {
            for face in &card.faces {
                for ability in &face.abilities {
                    println!("  compiled: {}", serde_json::to_string(ability).unwrap());
                }
            }
        }
    }
}

#[rstest]
#[case("Rowan, Scion of War")]
#[case("Will, Scion of Peace")]
#[case("Bloodletter of Aclazotz")]
#[case("Thousand-Year Storm")]
#[case("Diligent Farmhand")]
#[case("Pardic Firecat")]
fn r4703_reads_back_whole(#[case] name: &str) {
    let report = r4703_report(name, &r4703_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::another_color("Rowan, Scion of War", r#"{ "Color": "Red" }"#, r#"{ "Color": "Green" }"#)]
#[case::life_gained("Rowan, Scion of War", r#""what": "LifeLost""#, r#""what": "LifeGainedTotal""#)]
#[case::opponents_lost("Rowan, Scion of War", r#""who": "You", "what""#, r#""who": "Opponents", "what""#)]
#[case::until_next_turn("Will, Scion of Peace", r#""UntilEndOfTurn""#, r#""UntilYourNextTurn""#)]
#[case::once("Will, Scion of Peace", r#", "per": { "Tallied": { "who": "You", "what": "LifeGainedTotal" } }"#, "")]
#[case::any_turn("Bloodletter of Aclazotz", r#", "condition": "YourTurn""#, "")]
#[case::you("Bloodletter of Aclazotz", r#""who": "Opponents""#, r#""who": "You""#)]
#[case::thrice("Bloodletter of Aclazotz", r#""LossTimes": 2"#, r#""LossTimes": 3"#)]
#[case::not_before("Thousand-Year Storm", r#", { "CastBefore": "Triggering" }"#, "")]
#[case::creatures("Thousand-Year Storm", r#"{ "Type": "Sorcery" }] }, {"#, r#"{ "Type": "Creature" }] }, {"#)]
#[case::another_name("Pardic Firecat", r#""CountsAsNamed": "Flame Burst""#, r#""CountsAsNamed": "Muscle Burst""#)]
#[case::from_exile("Diligent Farmhand", r#""from": "Graveyard""#, r#""from": "Exile""#)]
#[case::sacrificed_by_anyone("Korvold, Gleeful Glutton", r#""who": "You""#, r#""who": "EachPlayer""#)]
#[case::sacrificed_creatures(
    "Korvold, Gleeful Glutton",
    r#""filter": []"#,
    r#""filter": [{ "Type": "Creature" }]"#
)]
fn r4703_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r4703_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r4703_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}

/// The compiler reads each new wording into the ability the card's file holds for it.
#[rstest]
#[case("Rowan, Scion of War", 1)]
#[case("Will, Scion of Peace", 1)]
#[case("Bloodletter of Aclazotz", 1)]
#[case("Thousand-Year Storm", 0)]
#[case("Diligent Farmhand", 1)]
#[case("Pardic Firecat", 1)]
#[case("Korvold, Gleeful Glutton", 0)]
fn r4703_compiler_reads_the_wording(#[case] name: &str, #[case] ability: usize) {
    let card = common::printed()
        .into_iter()
        .find(|card| card.name == name)
        .unwrap();
    let compiled = mtg_cards::compile::compile(&card).card.unwrap();
    let file: mtg_cards::card::Card = serde_json::from_str(&r4703_file(name)).unwrap();
    assert_eq!(
        compiled.faces[0].abilities[ability], file.faces[0].abilities[ability],
        "{name}"
    );
}

/// A card the pack improved reads back on the lines it now writes, the rest by hand.
#[rstest]
#[case("Korvold, Gleeful Glutton", 0)]
fn r4703_partial_line_reads_back(#[case] name: &str, #[case] clause: usize) {
    let report = r4703_report(name, &r4703_file(name));
    let state = format!("{:?}", report.clauses[clause].state);
    assert!(state.starts_with("Reads"), "{name}: {:#?}", report.clauses);
}
