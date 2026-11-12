//! Goal: every card the r30-4c pack made fully automatic reads back as its printed text clause by clause, and each card
//! it left partial keeps by hand only the clauses it names.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use mtg_import::scryfall::cards;
use rstest::rstest;

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

/// Prints the clause states of each rules file in `R304C_RULES` checked against the record of the same name in
/// `R304C_FX`; a tool for writing rules files.
#[test]
fn probe() {
    let (Ok(rules), Ok(fx)) = (std::env::var("R304C_RULES"), std::env::var("R304C_FX")) else {
        return;
    };
    let all = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let mut entries: Vec<_> = std::fs::read_dir(&rules)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        let text = std::fs::read_to_string(&path).unwrap();
        let record_text =
            std::fs::read_to_string(std::path::Path::new(&fx).join(path.file_name().unwrap()))
                .unwrap();
        let Some(Ok(record)) = cards(format!("[{record_text}]").as_bytes()).next() else {
            continue;
        };
        let card = common::printed_of(&record);
        let report = check(&text, &card, &tokens, &names(&all), &Caps::everything());
        println!("== {}: {:?}", card.name, report.verdict());
        for problem in &report.problems {
            println!("  problem: {problem:?}");
        }
        for clause in &report.clauses {
            println!("  [{}] {} => {:?}", clause.face, clause.text, clause.state);
        }
        for stray in &report.strays {
            println!("  stray: {stray:?}");
        }
    }
}

/// Prints the wordings of each ability (one JSON per line) in the file `R304C_WORDS`; a tool for writing writers.
#[test]
fn words() {
    let Ok(file) = std::env::var("R304C_WORDS") else {
        return;
    };
    for line in std::fs::read_to_string(file)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        let ability: mtg_cards::ability::Ability = serde_json::from_str(line).unwrap();
        println!("== {line}");
        for text in mtg_cards::compile::written_variants(&ability) {
            println!("  {text}");
        }
    }
}

fn report_of(name: &str, text: &str) -> mtg_cards::proposal::Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    check(text, card, &tokens, &names(&cards), &Caps::everything())
}

fn file_of(name: &str) -> String {
    std::fs::read_to_string(common::cards_dir().join(format!("{}.json", slug(name)))).unwrap()
}

#[rstest]
#[case::the_mindskinner("The Mindskinner")]
#[case::swans_of_bryn_argoll("Swans of Bryn Argoll")]
#[case::gloom_surgeon("Gloom Surgeon")]
#[case::crumbling_sanctuary("Crumbling Sanctuary")]
#[case::undead_alchemist("Undead Alchemist")]
#[case::callous_giant("Callous Giant")]
#[case::fated_firepower("Fated Firepower")]
#[case::hawkeye_young_avenger("Hawkeye, Young Avenger")]
#[case::shield_of_the_avatar("Shield of the Avatar")]
#[case::invasion_of_regatha("Invasion of Regatha // Disciples of the Inferno")]
#[case::laezel("Lae'zel, Vlaakith's Champion")]
#[case::vizier_of_remedies("Vizier of Remedies")]
fn the_pack_cards_read_back_whole(#[case] name: &str) {
    let report = report_of(name, &file_of(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:?}",
        report.clauses
    );
}

/// A rules file with one change no longer reads back as the card: the writers say what the data says.
#[rstest]
#[case::the_mindskinner_your_own_mill(
    "The Mindskinner",
    r#""who": "Opponents", "count""#,
    r#""who": "You", "count""#
)]
#[case::gloom_surgeon_any_damage("Gloom Surgeon", r#", "combat": true"#, "")]
#[case::crumbling_sanctuary_to_you_only(
    "Crumbling Sanctuary",
    r#""to": "EachPlayer""#,
    r#""to": "You""#
)]
#[case::undead_alchemist_noncombat("Undead Alchemist", r#""combat": true"#, r#""combat": false"#)]
#[case::callous_giant_four("Callous Giant", r#""PreventAtMost": 3"#, r#""PreventAtMost": 4"#)]
#[case::hawkeye_combat("Hawkeye, Young Avenger", r#""combat": false"#, r#""combat": true"#)]
#[case::fated_firepower_charge("Fated Firepower", r#"{ "Other": "fire" }"#, r#""charge""#)]
#[case::shield_your_creatures_by_type(
    "Shield of the Avatar",
    r#"{ "ControlledBy": "You" }"#,
    r#"{ "ControlledBy": "Opponents" }"#
)]
#[case::regatha_plus_three(
    "Invasion of Regatha // Disciples of the Inferno",
    r#""Add": 2"#,
    r#""Add": 3"#
)]
#[case::laezel_plus_two("Lae'zel, Vlaakith's Champion", r#""Add": 1"#, r#""Add": 2"#)]
#[case::vizier_plus_one_counters("Vizier of Remedies", r#""-1/-1""#, r#""+1/+1""#)]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let text = file_of(name);
    assert!(text.contains(from), "{name} holds {from}");
    let report = report_of(name, &text.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name} with {to}");
}
