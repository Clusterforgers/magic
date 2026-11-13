//! Goal: every card the r40-7 pack made automatic reads back as its printed text clause by clause, and a near miss does not.
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

/// Prints the clause states of the cards named in `R407_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn probe() {
    let Ok(list) = std::env::var("R407_PROBE") else {
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

/// Prints what the compiler reads from the text `R407_TEXT` printed on the corpus card `R407_ON`; a tool for finding
/// the wordings the readers already know.
#[test]
fn compiled_text() {
    let (Ok(text), Ok(on)) = (std::env::var("R407_TEXT"), std::env::var("R407_ON")) else {
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

/// Prints the wordings the writers give each ability of the cards named in `R407_WRITE`; a tool for writing writers.
#[test]
fn written() {
    let Ok(list) = std::env::var("R407_WRITE") else {
        return;
    };
    for name in list.split('|') {
        let path = common::cards_dir().join(format!("{}.json", slug(name)));
        let card: mtg_cards::card::Card =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for face in &card.faces {
            for ability in &face.abilities {
                println!("== {name}");
                for text in mtg_cards::compile::written_variants(ability) {
                    println!("  {text}");
                }
            }
        }
    }
}

#[rstest]
#[case("Magmablood Archaic")]
#[case("Void Mirror")]
#[case("Rumbling Aftershocks")]
#[case("Rage Extractor")]
#[case("Nev, the Practical Dean")]
#[case("Adeline, Resplendent Cathar")]
#[case("Protean Hydra")]
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
#[case::any_mana("Void Mirror", r#""stat":"ColorsSpent""#, r#""stat":"ManaSpent""#)]
#[case::some_color("Void Mirror", r#""op":"Eq""#, r#""op":"Ge""#)]
#[case::its_colors(
    "Magmablood Archaic",
    r#""Triggering","stat":"ColorsSpent""#,
    r#""Triggering","stat":"Colors""#
)]
#[case::own_spent(
    "Magmablood Archaic",
    r#"{"Stat":{"of":"Triggering","stat":"ColorsSpent"}}"#,
    r#""ColorsSpent""#
)]
#[case::its_mana_value("Rumbling Aftershocks", r#""stat":"Kicks""#, r#""stat":"ManaValue""#)]
#[case::creature_spell("Rumbling Aftershocks", r#"["Kicked"]"#, r#"[{"Type":"Creature"}]"#)]
#[case::must_deal(
    "Rumbling Aftershocks",
    r#"{"May":{"effects":[{"DealDamage":{"amount":{"Stat":{"of":"Triggering","stat":"Kicks"}},"to":{"Target":0}}}]}}"#,
    r#"{"DealDamage":{"amount":{"Stat":{"of":"Triggering","stat":"Kicks"}},"to":{"Target":0}}}"#
)]
#[case::x_in_cost("Rage Extractor", r#""PhyrexianInCost""#, r#""XInCost""#)]
#[case::its_power("Rage Extractor", r#""stat":"ManaValue""#, r#""stat":"Power""#)]
#[case::opponents_cast("Rage Extractor", r#""by":"You""#, r#""by":"Opponents""#)]
#[case::second_x_spell("Nev, the Practical Dean", r#""nth":1"#, r#""nth":2"#)]
#[case::every_x_spell(
    "Nev, the Practical Dean",
    r#"{"Nth":{"event":{"SpellCast":{"by":"You","what":["XInCost"]}},"nth":1}}"#,
    r#"{"SpellCast":{"by":"You","what":["XInCost"]}}"#
)]
#[case::one_counter("Nev, the Practical Dean", r#""count":"X","#, "")]
#[case::only_that_player(
    "Adeline, Resplendent Cathar",
    r#"{"Union":[{"Bound":"opponent"},{"Each":[{"Type":"Planeswalker"},{"ControlledBy":{"Bound":"opponent"}}]}]}"#,
    r#"{"Bound":"opponent"}"#
)]
#[case::any_walker(
    "Adeline, Resplendent Cathar",
    r#"{"ControlledBy":{"Bound":"opponent"}}"#,
    r#"{"ControlledBy":"Opponents"}"#
)]
#[case::untapped("Adeline, Resplendent Cathar", r#""tapped":true,"#, "")]
#[case::one_counter_back("Protean Hydra", r#""count":2,"#, "")]
#[case::counter_put("Protean Hydra", r#"{"CountersRemoved""#, r#"{"CountersPut""#)]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {to}: {:#?}",
        report.clauses
    );
}
