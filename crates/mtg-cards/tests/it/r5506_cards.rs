//! Goal: every card the r55-06 pack made fully automatic reads back as its printed text clause by clause, a near miss
//! (another kind, another count, other sources) does not read back, and the compiler reads each new wording into the
//! ability the card holds.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r5506_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r5506_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r5506_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r5506_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R5506_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r5506_probe() {
    if let Ok(json) = std::env::var("R5506_JSON") {
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
    if let Ok(list) = std::env::var("R5506_COMPILE") {
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
    let Ok(list) = std::env::var("R5506_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r5506_report(name, &r5506_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r5506_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Boom Scholar")]
#[case("Dragonkin Berserker")]
#[case("Memory Crystal")]
#[case("Tectonic Split")]
fn r5506_reads_back_whole(#[case] name: &str) {
    let report = r5506_report(name, &r5506_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::scholar_boast(
    "Boom Scholar",
    r#""activating": "Exhaust""#,
    r#""activating": "Boast""#
)]
#[case::scholar_its_own(
    "Boom Scholar",
    r#"["Other", { "ControlledBy": "You" }]"#,
    r#"[{ "ControlledBy": "You" }]"#
)]
#[case::scholar_more("Boom Scholar", r#""change": "Reduce""#, r#""change": "Increase""#)]
#[case::berserker_exhaust(
    "Dragonkin Berserker",
    r#""activating": "Boast""#,
    r#""activating": "Exhaust""#
)]
#[case::berserker_goblins(
    "Dragonkin Berserker",
    r#"{ "Subtype": "Dragon" }"#,
    r#"{ "Subtype": "Goblin" }"#
)]
#[case::crystal_kicker(
    "Memory Crystal",
    r#""activating": "Buyback""#,
    r#""activating": "Morph""#
)]
#[case::split_down("Tectonic Split", r#""up": true"#, r#""up": false"#)]
#[case::split_creatures(
    "Tectonic Split",
    r#""filter": [{ "Type": "Land" }]"#,
    r#""filter": [{ "Type": "Creature" }]"#
)]
#[case::split_optional("Tectonic Split", r#""required": true"#, r#""required": false"#)]
#[case::crystal_yours(
    "Memory Crystal",
    r#""applies_to": []"#,
    r#""applies_to": [{ "ControlledBy": "You" }]"#
)]
fn r5506_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r5506_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r5506_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}

/// The compiler reads each card's cost change, and every ability it reads is one the card's own file holds.
#[rstest]
#[case("Boom Scholar")]
#[case("Dragonkin Berserker")]
#[case("Memory Crystal")]
#[case("Tectonic Split")]
fn r5506_the_compiler_reads_the_card(#[case] name: &str) {
    let card = common::printed()
        .into_iter()
        .find(|card| card.name == name)
        .unwrap();
    let compiled = mtg_cards::compile::compile(&card).card.unwrap();
    let file: mtg_cards::card::Card = serde_json::from_str(&r5506_file(name)).unwrap();
    let read: Vec<&Ability> = compiled
        .faces
        .iter()
        .flat_map(|f| &f.abilities)
        .filter(|ability| !ability.is_manual())
        .collect();
    let held: Vec<&Ability> = file.faces.iter().flat_map(|f| &f.abilities).collect();
    assert!(
        read.iter().all(|ability| held.contains(ability)),
        "{name}: {read:?}"
    );
    assert!(
        read.iter().any(|ability| matches!(
            ability,
            Ability::CostModifier { .. } | Ability::AdditionalCost { .. }
        )),
        "{name}: the cost is read"
    );
}
