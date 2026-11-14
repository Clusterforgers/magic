//! Goal: every card the r60-07 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another count, another player, a clause dropped) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;
use rstest::rstest;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;

fn r6007_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r6007_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r6007_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r6007_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R6007_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r6007_probe() {
    if let Ok(json) = std::env::var("R6007_JSON") {
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
    if let Ok(list) = std::env::var("R6007_COMPILE") {
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
    let Ok(list) = std::env::var("R6007_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r6007_report(name, &r6007_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r6007_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Blinding Beam")]
#[case("Breathstealer's Crypt")]
#[case("Zur's Weirding")]
#[case("River Song")]
#[case("Theorist's Sanctum")]
#[case("Way of the Mind Sculptor")]
#[case("Destined Confrontation")]
#[case("Slaughter the Strong")]
#[case("Elvish Refueler")]
#[case("Dragon's Disciple")]
fn r6007_reads_back_whole(#[case] name: &str) {
    let report = r6007_report(name, &r6007_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

/// Cards that stay `Partial` read back clause by clause but for their `Manual` clauses.
#[rstest]
#[case("Jace, Reality Sculptor", 1)]
#[case("Illithid Harvester // Plant Tadpoles", 1)]
fn r6007_reads_back_but_the_manual_clauses(#[case] name: &str, #[case] manual: usize) {
    let report = r6007_report(name, &r6007_file(name));
    let left = report
        .clauses
        .iter()
        .filter(|clause| format!("{:?}", clause.state).starts_with("Manual"))
        .count();
    assert_eq!(left, manual, "{name}: {:#?}", report.clauses);
    assert!(
        report.clauses.iter().all(|clause| {
            let state = format!("{:?}", clause.state);
            state.starts_with("Manual") || state.starts_with("Reads")
        }),
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::beam_your_creatures("Blinding Beam", r#""whose": { "Target": 0 }"#, r#""whose": "You""#)]
#[case::beam_lands(
    "Blinding Beam",
    r#""what": [{ "Type": "Creature" }],
                                "whose""#,
    r#""what": [{ "Type": "Land" }],
                                "whose""#
)]
#[case::crypt_two_life("Breathstealer's Crypt", r#""PayLife": 3"#, r#""PayLife": 2"#)]
#[case::crypt_lands(
    "Breathstealer's Crypt",
    r#"[{ "Type": "Creature" }]"#,
    r#"[{ "Type": "Land" }]"#
)]
#[case::weirding_three_life("Zur's Weirding", r#""PayLife": 2"#, r#""PayLife": 3"#)]
#[case::river_song_top(
    "River Song",
    r#""R6007DrawFromBottom": "You""#,
    r#""R6007DrawFromBottom": "Opponents""#
)]
#[case::river_song_you_scry(
    "River Song",
    r#""action": "Scried", "who": "Opponents""#,
    r#""action": "Scried", "who": "You""#
)]
#[case::sanctum_dragon(
    "Theorist's Sanctum",
    r#""R6007BeholdOrTapped": [{ "Subtype": "Jace" }]"#,
    r#""R6007BeholdOrTapped": [{ "Subtype": "Dragon" }]"#
)]
#[case::sanctum_three("Theorist's Sanctum", r#""count": 2"#, r#""count": 3"#)]
#[case::way_one_counter("Way of the Mind Sculptor", r#""right": 2"#, r#""right": 1"#)]
#[case::destined_five("Destined Confrontation", r#""most": 4"#, r#""most": 5"#)]
#[case::refueler_opponents(
    "Elvish Refueler",
    r#""R6007ExhaustAgain": "You""#,
    r#""R6007ExhaustAgain": "Opponents""#
)]
#[case::disciple_two("Dragon's Disciple", r#""count": 1"#, r#""count": 2"#)]
#[case::disciple_angel(
    "Dragon's Disciple",
    r#""of": [{ "Subtype": "Dragon" }]"#,
    r#""of": [{ "Subtype": "Angel" }]"#
)]
fn r6007_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r6007_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r6007_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
