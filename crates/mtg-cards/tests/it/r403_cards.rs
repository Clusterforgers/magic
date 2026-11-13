//! Goal: every card the r40-3 pack made fully automatic reads back as its printed text clause by clause, the compiler
//! reads its new wordings into the same rules, and a near miss does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r403_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r403_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r403_card(name: &str) -> PrintedCard {
    common::printed()
        .into_iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"))
}

fn r403_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = r403_card(name);
    check(
        text,
        &card,
        &tokens,
        &r403_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the compiler's reading and the clause states of the cards named in `R403_PROBE` (separated by `|`) and the
/// wordings of the abilities in `R403_JSON` (one per line); a tool for writing rules files.
#[test]
fn r403_probe() {
    if let Ok(json) = std::env::var("R403_JSON") {
        for line in json.split('\n').filter(|line| !line.trim().is_empty()) {
            let ability: Ability = serde_json::from_str(line).unwrap();
            println!("-- {line}");
            for text in mtg_cards::compile::written_variants(&ability)
                .iter()
                .take(6)
            {
                println!("  written: {text}");
            }
        }
    }
    if let Ok(texts) = std::env::var("R403_TEXT") {
        for line in texts.split('\n').filter(|line| !line.trim().is_empty()) {
            let (name, text) = line.split_once("=>").unwrap();
            let mut card = r403_card(name);
            card.faces[0].oracle_text = text.to_owned();
            let compiled = mtg_cards::compile::compile(&card);
            println!("-- {text}");
            for ability in compiled
                .card
                .iter()
                .flat_map(|card| &card.faces[0].abilities)
            {
                println!("  compiled: {}", serde_json::to_string(ability).unwrap());
            }
        }
    }
    let Ok(list) = std::env::var("R403_PROBE") else {
        return;
    };
    for name in list.split('|').filter(|name| !name.is_empty()) {
        let compiled = mtg_cards::compile::compile(&r403_card(name));
        if let Some(card) = &compiled.card {
            for face in &card.faces {
                for ability in &face.abilities {
                    println!("  compiled: {}", serde_json::to_string(ability).unwrap());
                }
            }
        }
        let Ok(file) =
            std::fs::read_to_string(common::cards_dir().join(format!("{}.json", slug(name))))
        else {
            continue;
        };
        let report = r403_report(name, &file);
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
        let card: mtg_cards::card::Card = serde_json::from_str(&file).unwrap();
        for face in &card.faces {
            for ability in &face.abilities {
                for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                    println!("  written: {text}");
                }
            }
        }
    }
}

#[rstest]
#[case("Stunning Strike")]
#[case("Denry Klin, Editor in Chief")]
#[case("Voidpouncer")]
#[case("Grimdancer")]
#[case("Far Fortune, End Boss")]
#[case("Aether Revolt")]
#[case("Anthem of Rakdos")]
#[case("Thought Lash")]
#[case("Hibernation's End")]
#[case("Role Reversal")]
#[case("Legerdemain")]
#[case("Fall of Cair Andros")]
#[case("Magmatic Galleon")]
#[case("Maarika, Brutal Gladiator")]
#[case("Rith, Liberated Primeval")]
#[case("Heart of Bogardan")]
#[case("Shah of Naar Isle")]
fn r403_reads_back_whole(#[case] name: &str) {
    let report = r403_report(name, &r403_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::legendary_only(
    "Stunning Strike",
    r#"{ "Not": { "Supertype": "Legendary" } }"#,
    r#"{ "Supertype": "Legendary" }"#
)]
#[case::no_vigilance("Denry Klin, Editor in Chief", r#", { "Other": "vigilance" }"#, "")]
#[case::two_counters_needed(
    "Denry Klin, Editor in Chief",
    r#""op": "Ge", "right": 1"#,
    r#""op": "Ge", "right": 2"#
)]
#[case::two_trample_counters(
    "Voidpouncer",
    r#"{ "Other": "trample" }, "count": 1 }"#,
    r#"{ "Other": "trample" }, "count": 2 }"#
)]
#[case::a_flying_counter("Voidpouncer", r#"{ "Other": "trample" }"#, r#"{ "Other": "flying" }"#)]
#[case::three_kinds("Grimdancer", r#""count": 2"#, r#""count": 3"#)]
#[case::plus_two("Far Fortune, End Boss", r#""Add": 1"#, r#""Add": 2"#)]
#[case::plus_one("Aether Revolt", r#""Add": 2"#, r#""Add": 1"#)]
#[case::triple("Anthem of Rakdos", r#""Multiply": 2"#, r#""Multiply": 3"#)]
#[case::paid("Thought Lash", "DeclinedCumulativeUpkeep", "PaidCumulativeUpkeep")]
#[case::declined(
    "Hibernation's End",
    "PaidCumulativeUpkeep",
    "DeclinedCumulativeUpkeep"
)]
#[case::without_battles("Role Reversal", "\"Battle\",\n", "")]
#[case::thrice_the_counters("Heart of Bogardan", r#""by": 2"#, r#""by": 3"#)]
#[case::when_paid(
    "Heart of Bogardan",
    "DeclinedCumulativeUpkeep",
    "PaidCumulativeUpkeep"
)]
#[case::upkeep_paid("Shah of Naar Isle", "PaidEcho", "PaidCumulativeUpkeep")]
#[case::artifacts_only(
    "Legerdemain",
    r#""types": ["Artifact", "Creature"]"#,
    r#""types": ["Artifact"]"#
)]
fn r403_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r403_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r403_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
