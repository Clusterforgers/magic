//! Goal: every card the r55-03 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another zone, a limit or rider dropped, another cost) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r5503_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r5503_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r5503_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r5503_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R5503_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r5503_probe() {
    if let Ok(json) = std::env::var("R5503_JSON") {
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
    if let Ok(list) = std::env::var("R5503_COMPILE") {
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
    let Ok(list) = std::env::var("R5503_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r5503_report(name, &r5503_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r5503_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Intrepid Paleontologist")]
#[case("Osteomancer Adept")]
#[case("Gix, Yawgmoth Praetor")]
#[case("Reenact the Crime")]
#[case("Nashi, Moon Sage's Scion")]
#[case("Grip of Chaos")]
fn r5503_reads_back_whole(#[case] name: &str) {
    let report = r5503_report(name, &r5503_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::paleontologist_any_owner("Intrepid Paleontologist", r#"{ "OwnedBy": "You" },"#, "")]
#[case::paleontologist_no_finality(
    "Intrepid Paleontologist",
    r#""riders": [{ "EntersWithCounter": { "Other": "finality" } }]"#,
    r#""riders": []"#
)]
#[case::osteomancer_without_forage("Osteomancer Adept", r#""additional": ["Forage"],"#, "")]
#[case::osteomancer_for_ever(
    "Osteomancer Adept",
    r#""duration": "UntilEndOfTurn""#,
    r#""duration": "Forever""#
)]
#[case::anchor_any_turn(
    "The Temporal Anchor",
    r#""when": "YourTurn""#,
    r#""when": { "Not": "YourTurn" }"#
)]
#[case::gix_paying("Gix, Yawgmoth Praetor", r#", "free": true"#, "")]
#[case::gix_no_lands("Gix, Yawgmoth Praetor", r#", "play": true"#, "")]
#[case::reenact_from_the_battlefield(
    "Reenact the Crime",
    r#""ArrivedThisTurn""#,
    r#""GravedThisTurn""#
)]
#[case::reenact_lands_too("Reenact the Crime", r#"{ "Not": { "Type": "Land" } },"#, "")]
#[case::nashi_two_cards("Nashi, Moon Sage's Scion", r#""limit": 1"#, r#""limit": 2"#)]
#[case::nashi_paying_mana("Nashi, Moon Sage's Scion", r#""PayLife""#, r#""PayEnergy""#)]
#[case::grip_another_target("Grip of Chaos", r#""Random""#, r#""Another""#)]
#[case::grip_spells_only("Grip of Chaos", r#""PutOnStack": {}"#, r#""SpellCast": {}"#)]
fn r5503_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r5503_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r5503_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
