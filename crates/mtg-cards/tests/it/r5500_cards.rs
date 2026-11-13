//! Goal: every card the r55-00 pack made fully automatic reads back as its printed text clause by clause, the compiler
//! reads each new wording into the file's ability, and a near miss of each new wording (another limit, another counter,
//! another turn) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r5500_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r5500_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r5500_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r5500_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states, the written variants and the compiler's reading of the cards named in `R5500_PROBE`
/// (separated by `|`); a tool for writing rules files.
#[test]
fn r5500_probe() {
    if let Ok(json) = std::env::var("R5500_JSON") {
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
    let Ok(list) = std::env::var("R5500_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r5500_report(name, &r5500_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r5500_file(name)).unwrap();
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
#[case("As Foretold")]
#[case("Liesa, Shroud of Dusk")]
#[case("Obeka, Splitter of Seconds")]
#[case("Kenessos, Priest of Thassa")]
#[case("Jandor's Ring")]
#[case("Cyclonus, the Saboteur // Cyclonus, Cybertronian Fighter")]
fn r5500_reads_back_whole(#[case] name: &str) {
    let report = r5500_report(name, &r5500_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::your_turns_only("As Foretold", r#""any_turn": true,"#, "")]
#[case::every_time("As Foretold", r#""once": true,"#, "")]
#[case::charge_counters("As Foretold", r#""time""#, r#""charge""#)]
#[case::or_greater("As Foretold", r#""op": "Le""#, r#""op": "Ge""#)]
#[case::for_one("As Foretold", r#""rather": "{0}""#, r#""rather": "{1}""#)]
#[case::one_upkeep("Obeka, Splitter of Seconds", r#""count": "Triggering","#, "")]
#[case::after_this_step(
    "Obeka, Splitter of Seconds",
    r#""after_phase": true"#,
    r#""after_phase": false"#
)]
#[case::draw_steps(
    "Obeka, Splitter of Seconds",
    r#""step": "Upkeep""#,
    r#""step": "Draw""#
)]
#[case::combat_phase(
    "Cyclonus, the Saboteur // Cyclonus, Cybertronian Fighter",
    r#""phases": ["Beginning"]"#,
    r#""phases": ["Combat"]"#
)]
#[case::whatever_happens(
    "Cyclonus, the Saboteur // Cyclonus, Cybertronian Fighter",
    r#""condition": "Performed","#,
    r#""condition": "YourTurn","#
)]
#[case::plus_two("Kenessos, Priest of Thassa", r#""Add": 1"#, r#""Add": 2"#)]
#[case::not_serpents(
    "Kenessos, Priest of Thassa",
    r#"{ "Subtype": "Serpent" }"#,
    r#"{ "Subtype": "Fish" }"#
)]
#[case::to_hand(
    "Kenessos, Priest of Thassa",
    r#""to": { "Battlefield": {} }"#,
    r#""to": "Hand""#
)]
#[case::any_drawn_card("Jandor's Ring", r#""LastDrawnThisTurn""#, r#""DrawnThisTurn""#)]
#[case::three_mana("Jandor's Ring", r#""{2}""#, r#""{3}""#)]
#[case::three_life("Liesa, Shroud of Dusk", r#""TaxAsLife": 2"#, r#""TaxAsLife": 3"#)]
fn r5500_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r5500_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r5500_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}

/// The compiler reads each new wording into the ability the card's file holds for it.
#[rstest]
#[case("As Foretold", 0, 1)]
#[case("Liesa, Shroud of Dusk", 0, 0)]
fn r5500_compiler_reads_the_wording(
    #[case] name: &str,
    #[case] face: usize,
    #[case] ability: usize,
) {
    let card = common::printed()
        .into_iter()
        .find(|card| card.name == name)
        .unwrap();
    let compiled = mtg_cards::compile::compile(&card).card.unwrap();
    let file: mtg_cards::card::Card = serde_json::from_str(&r5500_file(name)).unwrap();
    assert_eq!(
        compiled.faces[face].abilities[ability], file.faces[face].abilities[ability],
        "{name}"
    );
}
