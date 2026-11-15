//! Goal: every card the r64-03 pack made fully automatic reads back as its printed text clause by clause, and a near
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

fn r6403_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r6403_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r6403_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r6403_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R6403_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r6403_probe() {
    if let Ok(json) = std::env::var("R6403_JSON") {
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
    if let Ok(list) = std::env::var("R6403_COMPILE") {
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
    let Ok(list) = std::env::var("R6403_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r6403_report(name, &r6403_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r6403_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Gisela, the Broken Blade")]
#[case("Titania, Voice of Gaea")]
#[case("Vanille, Cheerful l'Cie")]
#[case("Graf Rats")]
#[case("Urza, Lord Protector")]
#[case("Hanweir Battlements")]
#[case("Brisela, Voice of Nightmares")]
#[case("Bruna, the Fading Light")]
#[case("Chittering Host")]
#[case("Fang, Fearless l'Cie")]
#[case("Hanweir Garrison")]
#[case("Hanweir, the Writhing Township")]
#[case("Midnight Scavengers")]
#[case("Argoth, Sanctum of Nature")]
#[case("Arcanum Wings")]
fn r6403_reads_back_whole(#[case] name: &str) {
    let report = r6403_report(name, &r6403_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::gisela_other_partner(
    "Gisela, the Broken Blade",
    r#""Named": "Bruna, the Fading Light" }]"#,
    r#""Named": "Bruna" }]"#
)]
#[case::gisela_other_result(
    "Gisela, the Broken Blade",
    r#""into": "Brisela, Voice of Nightmares""#,
    r#""into": "Brisela""#
)]
#[case::gisela_any_owner(
    "Gisela, the Broken Blade",
    r#"["This", { "OwnedBy": "You" }, { "ControlledBy": "You" }]"#,
    r#"["This", { "ControlledBy": "You" }]"#
)]
#[case::gisela_other_step("Gisela, the Broken Blade", r#""step": "End""#, r#""step": "Upkeep""#)]
#[case::titania_three_lands("Titania, Voice of Gaea", r#""right": 4"#, r#""right": 3"#)]
#[case::titania_a_creature(
    "Titania, Voice of Gaea",
    r#"{ "Type": "Land" }, { "Named""#,
    r#"{ "Type": "Creature" }, { "Named""#
)]
#[case::vanille_free("Vanille, Cheerful l'Cie", r#""{3}{B}{G}""#, r#""{3}{G}""#)]
#[case::rats_each_combat("Graf Rats", r#""whose": "You""#, r#""whose": "EachPlayer""#)]
#[case::urza_instant("Urza, Lord Protector", r#""timing": "Sorcery","#, "")]
#[case::urza_a_creature(
    "Urza, Lord Protector",
    r#"{ "Type": "Artifact" }, { "Named": "The"#,
    r#"{ "Type": "Creature" }, { "Named": "The"#
)]
#[case::battlements_untapped(
    "Hanweir Battlements",
    r#"[{ "Mana": "{3}{R}{R}" }, "Tap"]"#,
    r#""{3}{R}{R}""#
)]
#[case::brisela_four("Brisela, Voice of Nightmares", r#""to": 3"#, r#""to": 4"#)]
#[case::brisela_everyone(
    "Brisela, Voice of Nightmares",
    r#""who": "Opponents""#,
    r#""who": "EachPlayer""#
)]
#[case::wings_other_cost("Arcanum Wings", r#""AuraSwap": "{2}{U}""#, r#""AuraSwap": "{1}{U}""#)]
fn r6403_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r6403_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r6403_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
