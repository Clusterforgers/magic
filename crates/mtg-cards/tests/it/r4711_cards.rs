//! Goal: every card the r47-11 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another draft note, a note dropped, another comparison) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r4711_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r4711_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r4711_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r4711_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R4711_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r4711_probe() {
    if let Ok(json) = std::env::var("R4711_JSON") {
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
    if let Ok(list) = std::env::var("R4711_COMPILE") {
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
    let Ok(list) = std::env::var("R4711_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r4711_report(name, &r4711_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r4711_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Lurking Automaton")]
#[case("Pyretic Hunter")]
#[case("Cogwork Grinder")]
#[case("Custodi Peacekeeper")]
#[case("Paliano Vanguard")]
#[case("Spire Phantasm")]
#[case("Smuggler Captain")]
#[case("Garbage Fire")]
#[case("Regicide")]
#[case("Noble Banneret")]
#[case("Cogwork Tracker")]
#[case("Spawnsire of Ulamog")]
#[case("Clergy of the Holy Nimbus")]
#[case("Arcane Savant")]
#[case("Volatile Chimera")]
#[case("Aether Searcher")]
#[case("Spiny Starfish")]
fn r4711_reads_back_whole(#[case] name: &str) {
    let report = r4711_report(name, &r4711_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::removed_not_noted("Lurking Automaton", r#""HighestNumber""#, r#""CardsRemoved""#)]
#[case::noted_not_removed("Cogwork Grinder", r#""CardsRemoved""#, r#""HighestNumber""#)]
#[case::power_at_least("Custodi Peacekeeper", r#""op": "Le""#, r#""op": "Ge""#)]
#[case::a_name_not_a_type("Paliano Vanguard", r#""NotedType""#, r#""NotedName""#)]
#[case::without_the_note("Paliano Vanguard", r#"{ "Drafted": "NotedType" },"#, "")]
#[case::a_type_not_a_name("Smuggler Captain", r#""NotedName""#, r#""NotedType""#)]
#[case::colors_not_name("Regicide", r#""ChosenColors""#, r#""NotedName""#)]
#[case::another_draft_rule("Regicide", r#""ChooseColorsAround""#, r#""NoteName""#)]
#[case::no_guess("Spire Phantasm", r#""if": { "Drafted": "GuessedRight" },"#, "")]
#[case::anyone_activates(
    "Clergy of the Holy Nimbus",
    r#""by": "Opponents""#,
    r#""by": "Anyone""#
)]
#[case::for_ever("Clergy of the Holy Nimbus", r#", "duration": "UntilEndOfTurn""#, "")]
#[case::banneret_without_lifelink(
    "Noble Banneret",
    r#"{ "AddKeyword": "Lifelink" }"#,
    r#"{ "AddKeyword": "Vigilance" }"#
)]
#[case::tracker_noted_name("Cogwork Tracker", r#""NotedPlayer""#, r#""GuessedRight""#)]
#[case::spawnsire_from_library(
    "Spawnsire of Ulamog",
    r#""zone": "Outside""#,
    r#""zone": "Library""#
)]
#[case::savant_creature("Arcane Savant", r#""ExileDraftedSpell""#, r#""ExileDraftedCreature""#)]
#[case::chimera_one_creature(
    "Volatile Chimera",
    r#""ExileDraftedCreatures""#,
    r#""ExileDraftedCreature""#
)]
#[case::chimera_until_end_of_turn("Volatile Chimera", r#""Forever""#, r#""UntilEndOfTurn""#)]
#[case::searcher_graveyard("Aether Searcher", r#""also": ["Hand"]"#, r#""also": ["Graveyard"]"#)]
#[case::starfish_sacrificed(
    "Spiny Starfish",
    r#""event": "Regenerated""#,
    r#""event": "Sacrificed""#
)]
#[case::starfish_its_controllers_end_step(
    "Spiny Starfish",
    r#"{ "step": "End" }"#,
    r#"{ "step": "End", "whose": "You" }"#
)]
fn r4711_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r4711_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r4711_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
