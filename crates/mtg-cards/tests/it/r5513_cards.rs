//! Goal: every card the r55-13 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another event, another subject, another condition) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r5513_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r5513_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r5513_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r5513_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R5513_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r5513_probe() {
    if let Ok(json) = std::env::var("R5513_JSON") {
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
    if let Ok(list) = std::env::var("R5513_COMPILE") {
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
    let Ok(list) = std::env::var("R5513_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r5513_report(name, &r5513_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r5513_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Karplusan Minotaur")]
#[case("Ramses, Assassin Lord")]
#[case("Silumgar Scavenger")]
#[case("Foe-Razer Regent")]
#[case("Neyith of the Dire Hunt")]
#[case("Cuombajj Witches")]
#[case("Nova Pentacle")]
fn r5513_reads_back_whole(#[case] name: &str) {
    let report = r5513_report(name, &r5513_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::minotaur_won_flip("Karplusan Minotaur", r#""LostCoinFlip""#, r#""Fought""#)]
#[case::minotaur_own_choice(
    "Karplusan Minotaur",
    r#"{ "ChosenBy": { "who": "Opponents", "what": "AnyTarget" } }"#,
    r#""AnyTarget""#
)]
#[case::ramses_any_attacker("Ramses, Assassin Lord", r#"{ "Subtype": "Assassin" }, "#, "")]
#[case::ramses_attacked_you(
    "Ramses, Assassin Lord",
    r#""player": "Triggering""#,
    r#""player": "You""#
)]
#[case::scavenger_always_haste(
    "Silumgar Scavenger",
    r#""Exploited": { "by": "This", "what": "Triggering" }"#,
    r#""Exploited": { "by": "Triggering", "what": "This" }"#
)]
#[case::regent_any_creature(
    "Foe-Razer Regent",
    r#"[{ "Type": "Creature" }, { "ControlledBy": "You" }]"#,
    r#"[{ "Type": "Creature" }]"#
)]
#[case::regent_one_counter("Foe-Razer Regent", r#""count": 2"#, r#""count": 1"#)]
#[case::regent_this_turn(
    "Foe-Razer Regent",
    r#""step": "End" }"#,
    r#""step": "End", "whose": "You" }"#
)]
#[case::neyith_opponents_creatures(
    "Neyith of the Dire Hunt",
    r#""Fought", "what": [{ "Type": "Creature" }, { "ControlledBy": "You" }]"#,
    r#""Fought", "what": [{ "Type": "Creature" }, { "ControlledBy": "Opponents" }]"#
)]
#[case::witches_both_yours(
    "Cuombajj Witches",
    r#"{ "ChosenBy": { "who": "Opponents", "what": "AnyTarget" } }"#,
    r#""AnyTarget""#
)]
#[case::pentacle_your_choice(
    "Nova Pentacle",
    r#""ChosenBy": { "who": "Opponents", "what": { "Object": [{ "Type": "Creature" }] } }"#,
    r#""Object": [{ "Type": "Creature" }]"#
)]
#[case::neyith_block_not_blocked("Neyith of the Dire Hunt", r#""BecomesBlocked""#, r#""Blocks""#)]
fn r5513_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r5513_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r5513_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
