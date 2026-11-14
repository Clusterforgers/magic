//! Goal: every card the r55-08 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another count, another kind of counter, another player) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r5508_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r5508_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r5508_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r5508_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R5508_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r5508_probe() {
    if let Ok(json) = std::env::var("R5508_JSON") {
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
    if let Ok(list) = std::env::var("R5508_COMPILE") {
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
    let Ok(list) = std::env::var("R5508_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r5508_report(name, &r5508_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r5508_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Way of the Mind Sculptor")]
#[case("Theorist's Sanctum")]
#[case("Resolute Veggiesaur")]
#[case("The Space Family Goblinson")]
#[case("River Song")]
#[case("Slaughter the Strong")]
#[case("Destined Confrontation")]
#[case("Pair o' Dice Lost")]
#[case("Blinding Beam")]
fn r5508_reads_back_whole(#[case] name: &str) {
    let report = r5508_report(name, &r5508_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::way_four_counters("Way of the Mind Sculptor", r#""count": 5"#, r#""count": 4"#)]
#[case::way_three_removed("Way of the Mind Sculptor", r#""right": 2"#, r#""right": 3"#)]
#[case::sanctum_reveals("Theorist's Sanctum", r#""BeholdOrTapped""#, r#""RevealOrTapped""#)]
#[case::sanctum_an_elf("Theorist's Sanctum", r#""Subtype": "Jace""#, r#""Subtype": "Elf""#)]
#[case::sanctum_three("Theorist's Sanctum", r#""count": 2"#, r#""count": 3"#)]
#[case::veggiesaur_second_die("Resolute Veggiesaur", r#""nth": 3"#, r#""nth": 2"#)]
#[case::goblinson_two_dice("The Space Family Goblinson", r#""right": 3"#, r#""right": 2"#)]
#[case::goblinson_spells(
    "The Space Family Goblinson",
    r#""what": "DiceRolled""#,
    r#""what": "SpellsCast""#
)]
#[case::slaughter_five_power(
    "Slaughter the Strong",
    r#""TotalPowerAtMost": 4"#,
    r#""TotalPowerAtMost": 5"#
)]
#[case::slaughter_mana_value(
    "Destined Confrontation",
    r#""TotalPowerAtMost""#,
    r#""TotalManaValueAtMost""#
)]
#[case::dice_three("Pair o' Dice Lost", r#""dice": 2"#, r#""dice": 3"#)]
#[case::dice_one_result("Pair o' Dice Lost", r#""Total""#, r#""Only""#)]
#[case::river_song_opponents_draw(
    "River Song",
    r#""DrawFromBottom": "You""#,
    r#""DrawFromBottom": "Opponents""#
)]
fn r5508_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r5508_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r5508_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
