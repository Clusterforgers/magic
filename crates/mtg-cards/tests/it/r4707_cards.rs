//! Goal: every card the r47-07 pack made fully automatic reads back as its printed text clause by clause, the compiler
//! reads its new wordings into the same rules, and a near miss (another kind of permanent, life left open, mana
//! abilities left out where the card covers them, another Licid cost) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r4707_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r4707_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r4707_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r4707_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R4707_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r4707_probe() {
    if let Ok(json) = std::env::var("R4707_JSON") {
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
    if let Ok(list) = std::env::var("R4707_COMPILE") {
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
    let Ok(list) = std::env::var("R4707_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r4707_report(name, &r4707_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r4707_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Stinging Licid")]
#[case("Dominating Licid")]
#[case("Transmogrifying Licid")]
#[case("Angel of Jubilation")]
#[case("Karn's Sylex")]
#[case("Yasharn, Implacable Earth")]
#[case("Fractured Powerstone")]
#[case("Ichor Elixir")]
#[case("Susan Foreman")]
#[case("Golgothian Sylex")]
#[case("Apocalypse Chime")]
#[case("City in a Bottle")]
#[case("Pia Nalaar, Chief Mechanic")]
fn r4707_reads_back_whole(#[case] name: &str) {
    let report = r4707_report(name, &r4707_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::licid_end_cost("Stinging Licid", r#""UntilPaid": "{U}""#, r#""UntilPaid": "{1}""#)]
#[case::licid_cost("Dominating Licid", r#""Mana": "{1}{U}{U}""#, r#""Mana": "{1}{U}""#)]
#[case::any_permanent(
    "Angel of Jubilation",
    r#""sacrifice": [{ "Type": "Creature" }]"#,
    r#""sacrifice": [{ "Type": "Artifact" }]"#
)]
#[case::life_open("Angel of Jubilation", r#""life": true, "#, "")]
#[case::mana_abilities_too("Karn's Sylex", r#", "but_mana_abilities": true"#, "")]
#[case::lands_too(
    "Yasharn, Implacable Earth",
    r#"{ "Not": { "Type": "Land" } }"#,
    r#"{ "Type": "Permanent" }"#
)]
#[case::opponents_only(
    "Yasharn, Implacable Earth",
    r#""who": "EachPlayer""#,
    r#""who": "Opponents""#
)]
#[case::chaos("Fractured Powerstone", r#""RollPlanarDie""#, r#""ChaosEnsues""#)]
#[case::any_time("Fractured Powerstone", r#""timing": "Sorcery","#, "")]
#[case::plain_dice("Ichor Elixir", r#""ExtraPlanarDie""#, r#""ExtraDie""#)]
#[case::any_amount("Pia Nalaar, Chief Mechanic", r#", { "XAtLeast": 1 }"#, "")]
#[case::two_or_more(
    "Pia Nalaar, Chief Mechanic",
    r#"{ "XAtLeast": 1 }"#,
    r#"{ "XAtLeast": 2 }"#
)]
#[case::crew_three("Pia Nalaar, Chief Mechanic", r#"{ "Crew": 2 }"#, r#"{ "Crew": 3 }"#)]
#[case::another_set("Golgothian Sylex", r#""Antiquities""#, r#""Homelands""#)]
#[case::tokens_too(
    "Golgothian Sylex",
    r#""filter": [{ "Not": "Token" }, "#,
    r#""filter": ["#
)]
#[case::regenerated("Apocalypse Chime", r#""regenerate": false"#, r#""regenerate": true"#)]
#[case::itself_too("City in a Bottle", r#""Other","#, "")]
#[case::lands_of_another_set(
    "City in a Bottle",
    r#""PlayLands": [{ "OriginallyPrintedIn": "ArabianNights" }]"#,
    r#""PlayLands": [{ "OriginallyPrintedIn": "Antiquities" }]"#
)]
fn r4707_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r4707_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r4707_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
