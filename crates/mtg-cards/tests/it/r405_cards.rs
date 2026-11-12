//! Goal: every card the r40-5 pack made automatic (mana spent only on spells of new kinds, equip abilities and disturb
//! costs) reads back as its printed text clause by clause, and a near miss does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, canonical, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn report(name: &str) -> mtg_cards::proposal::Report {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    report_of(name, &std::fs::read_to_string(&path).unwrap())
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

/// Prints the clause states of the cards named in `R405_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn probe() {
    let Ok(list) = std::env::var("R405_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = report(name);
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
    }
}

/// Prints what the compiler reads from the text `R405_TEXT` printed on the corpus card `R405_ON`; a tool for finding
/// the wordings the readers already know.
#[test]
fn compiled_text() {
    let (Ok(text), Ok(on)) = (std::env::var("R405_TEXT"), std::env::var("R405_ON")) else {
        return;
    };
    let mut card = common::printed()
        .into_iter()
        .find(|card| card.name == on)
        .unwrap();
    card.faces[0].oracle_text = text;
    let compiled = mtg_cards::compile::compile(&card);
    for clause in &compiled.clauses {
        println!("  {:?} read={} {}", clause.kind, clause.read, clause.text);
    }
    if let Some(card) = compiled.card {
        println!("{}", canonical(&card));
    }
}

#[rstest]
#[case("Titans' Nest")]
#[case("Unblinking Observer")]
#[case("Magewright's Stone")]
#[case("Break the Ice")]
#[case("Freya Crescent")]
#[case("Throne of Eldraine")]
#[case("Animal Attendant")]
#[case("Enthralling Hold")]
#[case("Dream Leash")]
#[case("Infinite Coursework")]
#[case("Biblioplex Tomekeeper")]
#[case("Vizier of the Anointed")]
#[case("Case File Auditor")]
fn reads_back_whole(#[case] name: &str) {
    let report = report(name);
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

/// A rules file of the corpus with one piece of its JSON replaced.
fn altered(name: &str, from: &str, to: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    let text = std::fs::read_to_string(&path).unwrap();
    let compact: serde_json::Value = serde_json::from_str(&text).unwrap();
    let compact = serde_json::to_string(&compact).unwrap();
    assert!(compact.contains(from), "{name} holds {from}");
    compact.replacen(from, to, 1)
}

#[rstest]
#[case::any_spell("Titans' Nest", r#""Colored","#, "")]
#[case::x_allowed("Titans' Nest", r#","NoXInCost""#, "")]
#[case::no_disturb("Unblinking Observer", r#""Disturb""#, r#""Flashback""#)]
#[case::any_creature("Magewright's Stone", r#",{"HasAbility":"TapCost"}"#, "")]
#[case::mana_ability("Magewright's Stone", r#""TapCost""#, r#""Mana""#)]
#[case::snow_only("Break the Ice", r#",{"CouldProduce":"Colorless"}"#, "")]
#[case::produce_green("Break the Ice", r#""CouldProduce":"Colorless""#, r#""CouldProduce":"Green""#)]
#[case::any_ability("Freya Crescent", r#"{"Ability":{"facts":["Equip"]}}"#, r#"{"Ability":{}}"#)]
#[case::multicolored("Throne of Eldraine", r#""Monocolored""#, r#""Colored""#)]
#[case::any_color("Throne of Eldraine", r#","chosen_color":true"#, "")]
#[case::any_mana_draws("Throne of Eldraine", r#","ChosenColorOnly""#, "")]
#[case::humans_too("Animal Attendant", r#","not_subtypes":["Human"]"#, "")]
#[case::not_elves("Animal Attendant", r#""Human""#, r#""Elf""#)]
#[case::tapped_hold("Enthralling Hold", r#""Untapped""#, r#""Tapped""#)]
#[case::tapped_leash("Dream Leash", r#""Untapped""#, r#""Tapped""#)]
#[case::prepared("Infinite Coursework", r#""Unprepare""#, r#""Prepare""#)]
#[case::two_unprepares("Biblioplex Tomekeeper", r#""Prepare""#, r#""Unprepare""#)]
#[case::embalm_only("Vizier of the Anointed", r#"{"HasKeywordNamed":"Eternalize"},"#, "")]
#[case::any_activation("Vizier of the Anointed", r#""ActivatedEternalizeOrEmbalm""#, r#""ActivatedExhaust""#)]
#[case::any_case_spell("Case File Auditor", r#""Subtype":"Case""#, r#""Subtype":"Clue""#)]
#[case::scry_on_solve("Case File Auditor", r#""SolvedCase""#, r#""Scried""#)]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {to}: {:#?}",
        report.clauses
    );
}

/// Ronin keeps its sacrifice of an attached Equipment by hand; its mana ability reads back alone.
#[test]
fn ronin_reads_its_mana_ability() {
    let report = report("Ronin, Shadow Stalker");
    assert!(
        matches!(
            report.clauses[0].state,
            mtg_cards::proposal::ClauseState::Reads { .. }
        ),
        "{:?}",
        report.clauses[0].state
    );
}
