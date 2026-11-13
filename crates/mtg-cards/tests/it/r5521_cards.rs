//! Goal: every card the r55-21 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another count, another dungeon, a condition dropped) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r5521_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        // Frodo's partner is a card outside the corpus.
        .chain(std::iter::once("sam, loyal attendant".to_owned()))
        .collect()
}

fn r5521_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r5521_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r5521_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R5521_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r5521_probe() {
    if let Ok(json) = std::env::var("R5521_JSON") {
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
    if let Ok(list) = std::env::var("R5521_COMPILE") {
        for name in list.split('|') {
            let card = common::printed()
                .into_iter()
                .find(|card| card.name == name)
                .unwrap();
            let compiled = mtg_cards::compile::compile(&card);
            println!("== {name}");
            for clause in &compiled.clauses {
                println!("  clause read={} {}", clause.read, clause.text);
            }
            if let Some(card) = compiled.card {
                for face in &card.faces {
                    for ability in &face.abilities {
                        println!("  {}", serde_json::to_string(ability).unwrap());
                    }
                }
            }
        }
    }
    let Ok(list) = std::env::var("R5521_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r5521_report(name, &r5521_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r5521_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Frodo, Adventurous Hobbit")]
#[case("Conspiracy Theorist")]
#[case("King Solomon's Frogs")]
#[case("Acererak the Archlich")]
#[case("Spikeshell Harrier")]
#[case("Breena, the Demagogue")]
#[case("Call the Spirit Dragons")]
#[case("Desert Warfare")]
fn r5521_reads_back_whole(#[case] name: &str) {
    let report = r5521_report(name, &r5521_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::frodo_three_temptations("Frodo, Adventurous Hobbit", r#""times": 2"#, r#""times": 3"#)]
#[case::frodo_four_life("Frodo, Adventurous Hobbit", r#""right": 3"#, r#""right": 4"#)]
#[case::theorist_land_cards(
    "Conspiracy Theorist",
    r#"{ "Not": { "Type": "Land" } }"#,
    r#"{ "Type": "Land" }"#
)]
#[case::theorist_without_if_you_do(
    "Conspiracy Theorist",
    r#""condition": "Performed""#,
    r#""condition": "YourTurn""#
)]
#[case::frogs_two_or_greater("King Solomon's Frogs", r#""to": 3"#, r#""to": 2"#)]
#[case::frogs_without_cast("King Solomon's Frogs", r#", { "WasCast": { "by": "You" } }"#, "")]
#[case::acererak_another_dungeon(
    "Acererak the Archlich",
    "Tomb of Annihilation",
    "Lost Mine of Phandelver"
)]
#[case::acererak_to_graveyard("Acererak the Archlich", r#""to": "Hand""#, r#""to": "Graveyard""#)]
#[case::acererak_each_player(
    "Acererak the Archlich",
    r#""over": "Opponents""#,
    r#""over": "EachPlayer""#
)]
#[case::harrier_any_player(
    "Spikeshell Harrier",
    r#""FastestPlayer": { "Bound": "foe" }"#,
    r#""FastestPlayer": "You""#
)]
#[case::harrier_without_condition(
    "Spikeshell Harrier",
    r#""condition": { "FastestPlayer""#,
    r#""condition": { "MaxSpeed""#
)]
#[case::breena_highest_life("Breena, the Demagogue", r#""LowestLife""#, r#""HighestLife""#)]
#[case::breena_you_draw("Breena, the Demagogue", r#""who": "Triggering""#, r#""who": "You""#)]
#[case::breena_one_counter("Breena, the Demagogue", r#""count": 2"#, r#""count": 1"#)]
#[case::dragons_four("Call the Spirit Dragons", r#""right": 5"#, r#""right": 4"#)]
#[case::dragons_no_green("Call the Spirit Dragons", r#""Green""#, r#""Red""#)]
#[case::desert_from_battlefield(
    "Desert Warfare",
    r#""from": "Library""#,
    r#""from": "Battlefield""#
)]
#[case::desert_opponents_end_step("Desert Warfare", r#""whose": "You""#, r#""whose": "Opponents""#)]
fn r5521_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r5521_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r5521_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
