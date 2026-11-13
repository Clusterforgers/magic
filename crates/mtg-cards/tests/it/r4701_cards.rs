//! Goal: every card the r47-01 pack made fully automatic reads back as its printed text clause by clause, the compiler
//! reads its new wordings into the same rules, and a near miss (another subtype beheld, another count tapped, a
//! permission from the hand, "your opponents" for the turn, another number of boasts) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r4701_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r4701_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r4701_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r4701_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R4701_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r4701_probe() {
    if let Ok(json) = std::env::var("R4701_JSON") {
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
    if let Ok(list) = std::env::var("R4701_COMPILE") {
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
    let Ok(list) = std::env::var("R4701_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r4701_report(name, &r4701_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r4701_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Molten Exhale")]
#[case("Tegwyll's Scouring")]
#[case("Haakon, Stromgald Scourge")]
#[case("Birgi, God of Storytelling // Harnfel, Horn of Bounty")]
#[case("Azor, the Lawbringer")]
fn r4701_reads_back_whole(#[case] name: &str) {
    let report = r4701_report(name, &r4701_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::behold_a_goblin("Molten Exhale", r#""Subtype": "Dragon""#, r#""Subtype": "Goblin""#)]
#[case::behold_and_exile(
    "Molten Exhale",
    r#"{ "Subtype": "Dragon" }] }"#,
    r#"{ "Subtype": "Dragon" }], "exile": true }"#
)]
#[case::no_flash("Molten Exhale", r#""flash": true"#, r#""flash": false"#)]
#[case::tap_two("Tegwyll's Scouring", r#""count": 3"#, r#""count": 2"#)]
#[case::without_flying("Tegwyll's Scouring", r#", { "HasKeyword": "Flying" }"#, "")]
#[case::anywhere(
    "Haakon, Stromgald Scourge",
    r#""selector": [{ "In": "Graveyard" }]"#,
    r#""selector": [{ "In": "Hand" }]"#
)]
#[case::boast_for_all(
    "Birgi, God of Storytelling // Harnfel, Horn of Bounty",
    r#""BoastTwice": "You""#,
    r#""BoastTwice": "EachPlayer""#
)]
#[case::this_turn("Azor, the Lawbringer", r#""TheirNextTurn""#, r#""UntilEndOfTurn""#)]
#[case::creature_spells(
    "Azor, the Lawbringer",
    r#"{ "AnyOf": [{ "Type": "Instant" }, { "Type": "Sorcery" }] }"#,
    r#"{ "Type": "Creature" }"#
)]
fn r4701_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r4701_file(name);
    assert!(file.contains(from), "{name} holds {from}");
    let report = r4701_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name} with {to}");
}

/// The compiler reads the new wordings into the abilities of the rules file at those places.
#[rstest]
#[case("Molten Exhale", &[0])]
#[case("Tegwyll's Scouring", &[0])]
#[case("Haakon, Stromgald Scourge", &[0, 1])]
#[case("Birgi, God of Storytelling // Harnfel, Horn of Bounty", &[1])]
fn r4701_compiler_reads_the_rules(#[case] name: &str, #[case] places: &[usize]) {
    let printed = common::printed()
        .into_iter()
        .find(|card| card.name == name)
        .unwrap();
    let compiled = mtg_cards::compile::compile(&printed).card.unwrap();
    let file: mtg_cards::card::Card = serde_json::from_str(&r4701_file(name)).unwrap();
    let read: Vec<&Ability> = compiled.faces.iter().flat_map(|f| &f.abilities).collect();
    for place in places {
        let wanted = &file.faces[0].abilities[*place];
        assert!(read.contains(&wanted), "{name}: {wanted:?} in {read:#?}");
    }
}
