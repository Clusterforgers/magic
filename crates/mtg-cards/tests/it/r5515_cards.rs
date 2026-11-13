//! Goal: every card the r55-15 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another host, another zone, another player, a step dropped) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r5515_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r5515_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r5515_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r5515_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R5515_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r5515_probe() {
    if let Ok(json) = std::env::var("R5515_JSON") {
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
    if let Ok(list) = std::env::var("R5515_COMPILE") {
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
    let Ok(list) = std::env::var("R5515_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r5515_report(name, &r5515_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r5515_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Bitterheart Witch")]
#[case("Curse of Misfortunes")]
#[case("Retether")]
#[case("Radiant Grace // Radiant Restraints")]
#[case("Soul Seizer // Ghastly Haunting")]
#[case("Lynde, Cheerful Tormentor")]
#[case("Maddening Hex")]
#[case("Kazuul, Tyrant of the Cliffs")]
#[case("Kang the Conqueror")]
#[case("Wonder Man, Hollywood Hero")]
#[case("Marvel Boy, Noh-Varr")]
#[case("Jack of Hearts, Volatile Hero")]
#[case("Ice Cave")]
#[case("Hulk, Gamma Goliath")]
fn r5515_reads_back_whole(#[case] name: &str) {
    let report = r5515_report(name, &r5515_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::witch_to_this("Bitterheart Witch", r#""to": { "Target": 0 }"#, r#""to": "This""#)]
#[case::witch_without_shuffle("Bitterheart Witch", r#", { "Shuffle": {} }"#, "")]
#[case::misfortunes_any_curse("Curse of Misfortunes", r#", { "Not": { "SharesName": { "Each": [{ "Subtype": "Curse" }, { "AttachedTo": "AttachedPlayer" }] } } }"#, "")]
#[case::misfortunes_to_you("Curse of Misfortunes", r#""to": "AttachedPlayer""#, r#""to": "You""#)]
#[case::retether_lands("Retether", r#""only": [{ "Type": "Creature" }]"#, r#""only": [{ "Type": "Land" }]"#)]
#[case::retether_hand("Retether", r#""In": "Graveyard""#, r#""In": "Hand""#)]
#[case::grace_untransformed("Radiant Grace // Radiant Restraints", r#""transformed": true, "#, "")]
#[case::grace_any_player("Radiant Grace // Radiant Restraints", r#""what": "Opponent""#, r#""what": "Player""#)]
fn r5515_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r5515_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r5515_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
