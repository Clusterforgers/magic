//! Goal: every card the r30-2b pack made automatic reads back as its printed text clause by clause, with no clause left to the table.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
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

/// Prints the compiler's reading and, where a rules file exists, the clause states of the cards named one per line in
/// the file `R302B_PROBE`; a tool for writing rules files.
#[test]
fn probe() {
    let Ok(file) = std::env::var("R302B_PROBE") else {
        return;
    };
    let wanted: Vec<String> = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    for card in cards.iter().filter(|card| wanted.contains(&card.name)) {
        println!("== {}", card.name);
        for face in &card.faces {
            println!("  text: {}", face.oracle_text.replace('\n', " / "));
        }
        let compiled = mtg_cards::compile::compile(card);
        if let Some(rules) = compiled.card {
            for f in &rules.faces {
                for ability in &f.abilities {
                    println!("  compiled: {}", serde_json::to_string(ability).unwrap());
                }
            }
        }
        let path = common::cards_dir().join(format!("{}.json", slug(&card.name)));
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let report = check(&text, card, &tokens, &names(&cards), &Caps::everything());
        println!("  verdict: {:?}", report.verdict());
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

fn report(name: &str) -> mtg_cards::proposal::Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    let text = std::fs::read_to_string(&path).unwrap();
    check(&text, card, &tokens, &names(&cards), &Caps::everything())
}

#[rstest]
#[case("Firecat Blitz")]
#[case("Flash Foliage")]
#[case("Hurkyl's Final Meditation")]
#[case("Interdict")]
#[case("Rust")]
#[case("Serpentine Spike")]
#[case("Soulblast")]
#[case("Teferi's Response")]
#[case("Trickbind")]
fn reads_back_whole(#[case] name: &str) {
    let report = report(name);
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

fn written(json: &str) -> Vec<String> {
    let ability: mtg_cards::ability::Ability = serde_json::from_str(json).unwrap();
    mtg_cards::compile::written_variants(&ability)
}

const RUST: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Ability":"Activated"},{"FromSource":[{"Type":"Artifact"}]}]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#;

/// The r30-2b wholes write the card's words and nothing near them: another source, another kind of ability or another
/// cost change is not written as the printed sentence.
#[rstest]
#[case::rust(RUST.to_owned(), "Counter target activated ability from an artifact source.", true)]
#[case::a_creature_source(RUST.replace("Artifact", "Creature"), "Counter target activated ability from an artifact source.", false)]
#[case::any_artifact_ability(RUST.replace(r#"{"Ability":"Activated"},"#, r#"{"AnyOf":[{"Ability":"Activated"},{"Ability":"Triggered"}]},"#), "Counter target activated ability from an artifact source.", false)]
#[case::the_type_on_the_ability(RUST.replace(r#"{"FromSource":[{"Type":"Artifact"}]}"#, r#"{"Type":"Artifact"}"#), "Counter target activated ability from an artifact source.", false)]
#[case::more_in_other_turns(r#"{"CostModifier":{"applies_to":["This"],"change":"Increase","mana":"{3}","condition":{"Not":"YourTurn"}}}"#.to_owned(), "During turns other than yours, this spell costs {3} more to cast.", true)]
#[case::more_in_your_turn(r#"{"CostModifier":{"applies_to":["This"],"change":"Increase","mana":"{3}","condition":"YourTurn"}}"#.to_owned(), "During turns other than yours, this spell costs {3} more to cast.", false)]
#[case::two_more(r#"{"CostModifier":{"applies_to":["This"],"change":"Increase","mana":"{2}","condition":{"Not":"YourTurn"}}}"#.to_owned(), "During turns other than yours, this spell costs {3} more to cast.", false)]
fn the_wholes_write_only_their_card(#[case] json: String, #[case] text: &str, #[case] reads: bool) {
    assert_eq!(
        written(&json).iter().any(|w| w == text),
        reads,
        "{:?}",
        written(&json)
    );
}
