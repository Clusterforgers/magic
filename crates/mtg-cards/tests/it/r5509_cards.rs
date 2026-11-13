//! Goal: every card the r55-09 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another rule, another fate of the pile, a count changed) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r5509_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r5509_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r5509_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r5509_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R5509_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r5509_probe() {
    if let Ok(json) = std::env::var("R5509_JSON") {
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
    if let Ok(list) = std::env::var("R5509_COMPILE") {
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
    let Ok(list) = std::env::var("R5509_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r5509_report(name, &r5509_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r5509_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Lurking Automaton")]
#[case("Pyretic Hunter")]
#[case("Cogwork Grinder")]
#[rstest]
#[case("Coalition Honor Guard")]
#[case("Standard Bearer")]
#[case("Coalition Flag")]
#[case("Ghastly Conscription")]
#[case("Become Anonymous")]
#[case("Jeskai Infiltrator")]
#[case("Triumph of Saint Katherine")]
#[case("Cosmic Intervention")]
#[case("Wildgrowth Archaic")]
#[case("Overclocked Electromancer")]
#[case("Braided Net // Braided Quipu")]
#[case("Elvish Refueler")]
#[case("The Foretold Soldier")]
fn r5509_reads_back_whole(#[case] name: &str) {
    let report = r5509_report(name, &r5509_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::flag_not_flagbearer(
    "Coalition Flag",
    r#""AddSubtype": "Flagbearer""#,
    r#""AddSubtype": "Soldier""#
)]
#[case::guard_another_rule(
    "Coalition Honor Guard",
    r#""Flagbearers""#,
    r#""GraveyardCardsUntargetable""#
)]
#[case::conscription_cloaks(
    "Ghastly Conscription",
    r#""then": "Manifest""#,
    r#""then": { "Cloak": {} }"#
)]
#[case::anonymous_untapped("Become Anonymous", r#""tapped": true"#, r#""tapped": false"#)]
#[case::anonymous_three_cards("Become Anonymous", r#""top": 2"#, r#""top": 3"#)]
#[case::infiltrator_two_cards("Jeskai Infiltrator", r#""top": 1"#, r#""top": 2"#)]
#[case::triumph_any_graveyard("Triumph of Saint Katherine", r#", { "OwnedBy": "You" }"#, "")]
#[case::triumph_not_whole("Triumph of Saint Katherine", r#", "whole": true"#, "")]
#[case::triumph_manifests(
    "Triumph of Saint Katherine",
    r#""then": "OnTop""#,
    r#""then": "Manifest""#
)]
#[case::cosmic_creatures_only(
    "Cosmic Intervention",
    r#"[{ "ControlledBy": "You" }]"#,
    r#"[{ "Type": "Creature" }, { "ControlledBy": "You" }]"#
)]
#[case::archaic_fixed_count(
    "Wildgrowth Archaic",
    r#""count": { "Stat": { "of": "Triggering", "stat": "ColorsSpent" } }"#,
    r#""count": 2"#
)]
#[case::electromancer_no_condition(
    "Overclocked Electromancer",
    r#""event": "DealtExcess" }"#,
    r#""event": "Damaged" }"#
)]
#[case::net_for_ever("Braided Net // Braided Quipu", r#""WhileTapped""#, r#""Forever""#)]
#[case::refueler_flagbearers("Elvish Refueler", r#""ExhaustAgain""#, r#""Flagbearers""#)]
#[case::soldier_exiled_face_up(
    "The Foretold Soldier",
    r#"{ "BecomeForetold": { "what": "This" } }"#,
    r#"{ "Exile": { "what": "This" } }"#
)]
fn r5509_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r5509_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r5509_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
