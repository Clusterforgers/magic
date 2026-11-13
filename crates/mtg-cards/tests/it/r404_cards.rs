//! Goal: every card the r40-4 pack made automatic reads back as its printed text clause by clause, and a near miss of
//! each new piece does not.
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

/// Prints the clause states of the cards named in `R404_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn probe() {
    let Ok(list) = std::env::var("R404_PROBE") else {
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

/// Prints what the compiler reads from the text `R404_TEXT` printed on the corpus card `R404_ON`; a tool for finding
/// the wordings the readers already know.
#[test]
fn compiled_text() {
    let (Ok(text), Ok(on)) = (std::env::var("R404_TEXT"), std::env::var("R404_ON")) else {
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

/// Prints every wording the writers give the ability JSON in `R404_WRITE`; a tool for writing rules files.
#[test]
fn written() {
    let Ok(json) = std::env::var("R404_WRITE") else {
        return;
    };
    let ability: mtg_cards::ability::Ability = serde_json::from_str(&json).unwrap();
    for text in mtg_cards::compile::written_variants(&ability) {
        println!("  W: {text}");
    }
}

#[rstest]
#[case("Rootwater Matriarch")]
#[case("Garland, Royal Kidnapper")]
#[case("Orcish Farmer")]
#[case("Arcum's Weathervane")]
#[case("Navigator's Compass")]
#[case("Gaea's Liege")]
#[case("Aven Soulgazer")]
#[case("Smoke Teller")]
#[case("Gauntlet of Power")]
#[case("Clara Oswald")]
#[case("Faceless One")]
#[case("The Prismatic Piper")]
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
#[case::for_good("Rootwater Matriarch", r#","duration":"WhileEnchanted""#, "")]
#[case::while_tapped("Rootwater Matriarch", r#""WhileEnchanted""#, r#""WhileSourceTapped""#)]
#[case::enchanted_artifact(
    "Rootwater Matriarch",
    r#"{"Type":"Creature"}"#,
    r#"{"Type":"Artifact"}"#
)]
#[case::while_you_are_monarch(
    "Garland, Royal Kidnapper",
    r#"{"WhileMonarch":"Triggering"}"#,
    r#"{"WhileMonarch":"You"}"#
)]
#[case::while_you_control_it(
    "Garland, Royal Kidnapper",
    r#"{"WhileMonarch":"Triggering"}"#,
    r#""WhileYouControlSource""#
)]
#[case::monarch_for_good(
    "Garland, Royal Kidnapper",
    r#","duration":{"WhileMonarch":"Triggering"}"#,
    ""
)]
#[case::until_your_next_turn(
    "Orcish Farmer",
    r#""UntilItsControllersNextUntapStep""#,
    r#""UntilYourNextTurn""#
)]
#[case::farmer_for_the_turn(
    "Orcish Farmer",
    r#""UntilItsControllersNextUntapStep""#,
    r#""UntilEndOfTurn""#
)]
#[case::snow_kept(
    "Arcum's Weathervane",
    r#"{"RemoveSupertype":"Snow"}"#,
    r#"{"AddSupertype":"Snow"}"#
)]
#[case::snow_for_the_turn("Arcum's Weathervane", r#""Forever""#, r#""UntilEndOfTurn""#)]
#[case::legendary_not_snow(
    "Arcum's Weathervane",
    r#"{"RemoveSupertype":"Snow"}"#,
    r#"{"RemoveSupertype":"Legendary"}"#
)]
#[case::any_snow_land("Arcum's Weathervane", r#"{"Not":{"Supertype":"Snow"}},"#, "")]
#[case::type_replaced(
    "Navigator's Compass",
    r#"{"AddLandTypeChosen":"chosen"}"#,
    r#"{"SetLandTypesChosen":"chosen"}"#
)]
#[case::compass_for_good("Navigator's Compass", r#""UntilEndOfTurn""#, r#""Forever""#)]
#[case::any_land("Navigator's Compass", r#",{"ControlledBy":"You"}"#, "")]
#[case::liege_while_controlled(
    "Gaea's Liege",
    r#""WhileSourceOnBattlefield""#,
    r#""WhileYouControlSource""#
)]
#[case::any_creature("Aven Soulgazer", r#","FaceDown""#, "")]
#[case::look_reveals(
    "Smoke Teller",
    r#"{"LookAtFaceDown":{"what":{"Target":0}}}"#,
    r#"{"Reveal":{"what":{"Target":0}}}"#
)]
#[case::any_color("Gauntlet of Power", r#""chosen_color":true,"#, "")]
#[case::nonbasic_too("Gauntlet of Power", r#",{"Supertype":"Basic"}]"#, "]")]
#[case::two_more("Gauntlet of Power", r#"{"ChosenColor":1}"#, r#"{"ChosenColor":2}"#)]
#[case::you_add("Gauntlet of Power", r#""who":"Triggering""#, r#""who":"You""#)]
#[case::chosen_as_it_enters("Faceless One", r#""SetCommanderColor""#, r#""SetChosenColor""#)]
#[case::only_on_the_battlefield("The Prismatic Piper", r#""defining":true,"#, "")]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {to}: {:#?}",
        report.clauses
    );
}
