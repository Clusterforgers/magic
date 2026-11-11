//! Goal: every card the rmt6 pack made fully automatic reads back as its printed text clause by clause, with no clause left to the table.
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

/// Prints the clause states of the cards named in `RMT6_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn probe() {
    let Ok(list) = std::env::var("RMT6_PROBE") else {
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

/// Writes, for each card named in the file `RMT6_COMPILE`, what the compiler reads where the rules file leaves a clause
/// to the table; a tool for writing rules files.
#[test]
fn compiled() {
    let Ok(file) = std::env::var("RMT6_COMPILE") else {
        return;
    };
    let wanted: Vec<String> = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    for card in common::printed() {
        if !wanted.contains(&card.name) {
            continue;
        }
        let compiled = mtg_cards::compile::compile(&card);
        let Some(rules) = compiled.card else { continue };
        for (face, f) in rules.faces.iter().enumerate() {
            for ability in &f.abilities {
                if !matches!(ability, mtg_cards::ability::Ability::Manual { .. }) {
                    println!(
                        "{}\t{face}\t{}",
                        card.name,
                        serde_json::to_string(ability).unwrap()
                    );
                }
            }
        }
    }
}

#[rstest]
#[case("Fog Bank")]
#[case("Electro, Assaulting Battery")]
#[case("Doc Samson, Super Psychiatrist")]
#[case("Forethought Amulet")]
#[case("Forsaken Monument")]
#[case("Brimaz, King of Oreskos")]
#[case("Corrupted Grafstone")]
#[case("Chromatic Orrery")]
#[case("Dynaheir, Invoker Adept")]
#[case("Fíli the Pathfinder")]
#[case("Bifur, Melodic Rider")]
#[case("Dáin, Lord of the Iron Hills")]
#[case("Aeve, Progenitor Ooze")]
#[case("Dragon Hunter")]
#[case("Archdemon of Paliano")]
#[case("Cogwork Spy")]
#[case("Canal Dredger")]
#[case("Deal Broker")]
#[case("Óin the Brave")]
#[case("Thorin Oakenshield")]
#[case("Kíli the Resourceful")]
fn reads_back_whole(#[case] name: &str) {
    let report = report(name);
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

const STORY: &str = r#"{"EnduringStory":"You"}"#;
const ANTHEM: &str = r#""affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]"#;
const TOKEN: &str = r#""affected":"This","mods":[{"RemoveSupertype":"Legendary"}]"#;
const DWARVES: &str = r#"{"TriggerAgain":{"source":[{"Subtype":"Dwarf"},{"ControlledBy":"You"}]}}"#;

fn written(json: &str) -> Vec<String> {
    let ability: mtg_cards::ability::Ability = serde_json::from_str(json).unwrap();
    mtg_cards::compile::written_variants(&ability)
}

/// The rmt6 writers write the card's words and nothing near them: another condition, another subject, another selector
/// or another rule is not written as the printed sentence.
#[rstest]
#[case::storied_anthem(format!(r#"{{"Static":{{{ANTHEM},"condition":{STORY}}}}}"#), "As long as you have an enduring story, creatures you control get +1/+1.", true)]
#[case::anthem_without_the_story(format!(r#"{{"Static":{{{ANTHEM}}}}}"#), "As long as you have an enduring story, creatures you control get +1/+1.", false)]
#[case::an_opponents_story(format!(r#"{{"Static":{{{ANTHEM},"condition":{{"EnduringStory":"Opponents"}}}}}}"#), "As long as you have an enduring story, creatures you control get +1/+1.", false)]
#[case::dwarves_again(format!(r#"{{"Rule":{{"rule":{DWARVES},"condition":{STORY}}}}}"#), "As long as you have an enduring story, if a triggered ability of a Dwarf you control triggers, that ability triggers an additional time.", true)]
#[case::every_dwarf_again(format!(r#"{{"Rule":{{"rule":{{"TriggerAgain":{{"source":[{{"Subtype":"Dwarf"}}]}}}},"condition":{STORY}}}}}"#), "As long as you have an enduring story, if a triggered ability of a Dwarf you control triggers, that ability triggers an additional time.", false)]
#[case::a_token_is_not_legendary(format!(r#"{{"Static":{{{TOKEN},"condition":{{"Matches":{{"subject":"This","selector":["Token"]}}}}}}}}"#), "~ isn't legendary if it's a token.", true)]
#[case::never_legendary(format!(r#"{{"Static":{{{TOKEN}}}}}"#), "~ isn't legendary if it's a token.", false)]
#[case::a_nontoken_is_not_legendary(format!(r#"{{"Static":{{{TOKEN},"condition":{{"Matches":{{"subject":"This","selector":[{{"Not":"Token"}}]}}}}}}}}"#), "~ isn't legendary if it's a token.", false)]
#[case::reach_for_dragons(r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":{"BlockAsThoughReach":[{"Subtype":"Dragon"}]}}}}}"#.to_owned(), "~ can block Dragons as though it had reach.", true)]
#[case::reach_for_birds(r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":{"BlockAsThoughReach":[{"Subtype":"Bird"}]}}}}}"#.to_owned(), "~ can block Dragons as though it had reach.", false)]
#[case::draft_face_up(r#"{"Rule":{"rule":{"Draft":"FaceUp"}}}"#.to_owned(), "Draft ~ face up.", true)]
#[case::draft_at_random_two(r#"{"Rule":{"rule":{"Draft":{"AtRandom":{"picks":2}}}}}"#.to_owned(), "As long as ~ is face up during the draft, you can't look at booster packs and must draft cards at random. After you draft three cards this way, turn ~ face down.", false)]
#[case::draft_at_random_three(r#"{"Rule":{"rule":{"Draft":{"AtRandom":{"picks":3}}}}}"#.to_owned(), "As long as ~ is face up during the draft, you can't look at booster packs and must draft cards at random. After you draft three cards this way, turn ~ face down.", true)]
#[case::draft_trade_is_not_face_up(r#"{"Rule":{"rule":{"Draft":"TradeAfterDraft"}}}"#.to_owned(), "Draft ~ face up.", false)]
fn rmt6_writers_write_the_printed_words(
    #[case] json: String,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written(&json);
    assert_eq!(found.iter().any(|w| w == text), reads, "{found:?}");
}
