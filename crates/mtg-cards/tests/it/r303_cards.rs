//! Goal: every card the r30-3 pack made fully automatic reads back as its printed text clause by clause, a near miss of
//! its rules does not, and a form a copy effect cannot make is refused.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, canonical, slug};
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

/// Prints the clause states of the cards named in `R303_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn probe() {
    let Ok(list) = std::env::var("R303_PROBE") else {
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

/// Writes the compiled rules file of each Scryfall record in the directory `R303_COMPILE` to the directory `R303_OUT`;
/// a tool for starting a rules file from what the compiler reads.
#[test]
fn compiled() {
    let (Ok(input), Ok(output)) = (std::env::var("R303_COMPILE"), std::env::var("R303_OUT")) else {
        return;
    };
    for entry in std::fs::read_dir(&input).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let Some(Ok(record)) = cards(format!("[{text}]").as_bytes()).next() else {
            continue;
        };
        let printed = common::printed_of(&record);
        let Some(card) = mtg_cards::compile::compile(&printed).card else {
            continue;
        };
        let out = std::path::Path::new(&output).join(path.file_name().unwrap());
        std::fs::write(out, canonical(&card)).unwrap();
    }
}

/// Checks each rules file of the directory `R303_CHECK` against the Scryfall record of the same file name in
/// `R303_FX` and prints its clause states; a tool for writing rules files of cards outside the corpus.
#[test]
fn check_outside() {
    let (Ok(rules), Ok(records)) = (std::env::var("R303_CHECK"), std::env::var("R303_FX")) else {
        return;
    };
    let corpus = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let mut entries: Vec<_> = std::fs::read_dir(&rules)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        let record = std::path::Path::new(&records).join(path.file_name().unwrap());
        let text = std::fs::read_to_string(&record).unwrap();
        let Some(Ok(record)) = cards(format!("[{text}]").as_bytes()).next() else {
            continue;
        };
        let printed = common::printed_of(&record);
        let mut known = names(&corpus);
        known.insert(printed.name.to_lowercase());
        let report = check(
            &std::fs::read_to_string(&path).unwrap(),
            &printed,
            &tokens,
            &known,
            &Caps::everything(),
        );
        println!("== {}: {:?}", printed.name, report.verdict());
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

#[rstest]
#[case("Back for More")]
#[case("Coalition Victory")]
#[case("Compleat Devotion")]
#[case("Defiant Stand")]
#[case("Glyph of Destruction")]
#[case("Last Stand")]
#[case("Providence")]
#[case("Psychic Rebuttal")]
#[case("Enkira, Hostile Scavenger")]
#[case("Pact Weapon")]
#[case("The Golden Throne")]
#[case("Archelos, Lagoon Mystic")]
#[case("Bronze Horse")]
#[case("Dance of the Manse")]
#[case("Monstrous War-Leech")]
#[case("Signal the Clans")]
#[case("Urborg Lhurgoyf")]
#[case("Begin the Invasion")]
#[case("Conflux")]
#[case("Corrupted Shapeshifter")]
#[case("Delirium")]
#[case("Harsh Justice")]
#[case("Invert // Invent")]
#[case("Kíli the Resourceful")]
#[case("New Perspectives")]
#[case("Primal Clay")]
#[case("Primal Plasma")]
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
#[case::any_creature(
    "Back for More",
    r#"{"Not":{"ControlledBy":"You"}}"#,
    r#"{"Type":"Creature"}"#
)]
#[case::four_colors(
    "Coalition Victory",
    r#""right":5}},{"Compare""#,
    r#""right":4}},{"Compare""#
)]
#[case::infect_not_toxic(
    "Compleat Devotion",
    r#"{"HasKeywordNamed":"Toxic"}"#,
    r#"{"HasKeyword":"Infect"}"#
)]
#[case::tap_not_untap(
    "Defiant Stand",
    r#"{"Untap":{"what":{"Target":0}}}"#,
    r#"{"Tap":{"what":{"Target":0}}}"#
)]
#[case::until_end_of_turn("Glyph of Destruction", r#""UntilEndOfCombat""#, r#""UntilEndOfTurn""#)]
#[case::three_per_swamp(
    "Last Stand",
    r#"{"by":2,"of":{"Count":[{"Subtype":"Swamp"}"#,
    r#"{"by":3,"of":{"Count":[{"Subtype":"Swamp"}"#
)]
#[case::twenty_life(
    "Providence",
    r#"{"SetLife":{"amount":26}}]}}"#,
    r#"{"SetLife":{"amount":20}}]}}"#
)]
#[case::one_card("Psychic Rebuttal", r#""right":2"#, r#""right":1"#)]
#[case::other_size(
    "Primal Plasma",
    r#"{"SetPT":{"power":3,"toughness":3}}"#,
    r#"{"SetPT":{"power":3,"toughness":2}}"#
)]
#[case::no_vigilance("Corrupted Shapeshifter", r#",{"AddKeyword":"Vigilance"}"#, "")]
#[case::no_wall("Primal Clay", r#"{"AddSubtype":"Wall"},"#, "")]
#[case::six_cards("New Perspectives", r#""right":7"#, r#""right":6"#)]
#[case::to_you("Harsh Justice", r#""to":{"ControllerOf":"Dealer"}"#, r#""to":"You""#)]
#[case::not_distinct("Begin the Invasion", r#""distinct":true,"#, "")]
#[case::not_story("Kíli the Resourceful", r#"{"EnduringStory":"You"}"#, r#""YourTurn""#)]
#[case::untapped_enters_tapped("Archelos, Lagoon Mystic", r#""by":"Untapped""#, r#""by":"Tapped""#)]
#[case::no_other_creature("Bronze Horse", r#""right":1"#, r#""right":2"#)]
#[case::five_or_more("Dance of the Manse", r#""right":6"#, r#""right":5"#)]
#[case::two_per_kick(
    "Urborg Lhurgoyf",
    r#"{"Mill":{"Times":{"by":3"#,
    r#"{"Mill":{"Times":{"by":2"#
)]
#[case::unkicked_mill("Monstrous War-Leech", r#","condition":"Kicked""#, "")]
#[case::two_names("Signal the Clans", r#""op":"Eq","right":3"#, r#""op":"Ge","right":2"#)]
#[case::three_zombies(
    "Enkira, Hostile Scavenger",
    r#""op":"Ge","right":2"#,
    r#""op":"Ge","right":3"#
)]
#[case::any_loss("Pact Weapon", r#"{"life_only":true,"who":"You"}"#, r#"{"who":"You"}"#)]
#[case::life_two(
    "The Golden Throne",
    r#"{"SetLife":{"amount":1}}"#,
    r#"{"SetLife":{"amount":2}}"#
)]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}

#[test]
fn a_form_that_a_copy_effect_cannot_make_is_refused() {
    let form = r#"{"name":"Primal Plasma","faces":[{"name":"Primal Plasma","abilities":[{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"BecomesChoice":[[{"ModifyPT":{"power":3,"toughness":3}},{"AddType":"Creature"}],[{"SetPT":{"power":2,"toughness":2}},{"AddType":"Creature"}]]}}}]}]}"#;
    let report = report_of("Primal Plasma", form);
    assert_eq!(report.verdict(), Verdict::Invalid, "{:?}", report.problems);
}
