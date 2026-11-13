//! Goal: every card the r55-23 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another turn's attacks, another defender rule, mana of any color) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r5523_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r5523_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r5523_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r5523_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R5523_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r5523_probe() {
    if let Ok(json) = std::env::var("R5523_JSON") {
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
    if let Ok(list) = std::env::var("R5523_COMPILE") {
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
    let Ok(list) = std::env::var("R5523_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r5523_report(name, &r5523_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r5523_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

/// Compiles the Scryfall objects at the paths in `R5523_SCRYFALL` (separated by `|`) and prints their clauses and
/// abilities; with `R5523_RULES` (a rules file per path, same order) also the checker's verdict; a tool for new cards.
#[test]
fn r5523_probe_outside() {
    let Ok(list) = std::env::var("R5523_SCRYFALL") else {
        return;
    };
    let rules: Vec<String> = std::env::var("R5523_RULES")
        .map(|rules| rules.split('|').map(str::to_owned).collect())
        .unwrap_or_default();
    let names = r5523_names(&common::printed());
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    for (at, path) in list.split('|').enumerate() {
        let text = std::fs::read_to_string(path).unwrap();
        let mut found: Vec<_> =
            mtg_import::scryfall::cards(format!("[{text}]").as_bytes()).collect();
        let card = common::printed_of(&found.remove(0).unwrap());
        let compiled = mtg_cards::compile::compile(&card);
        println!("== {}", card.name);
        for clause in &compiled.clauses {
            println!("  clause read={} {}", clause.read, clause.text);
        }
        if let Some(made) = compiled.card {
            for face in &made.faces {
                for ability in &face.abilities {
                    println!("  {}", serde_json::to_string(ability).unwrap());
                }
            }
        }
        if let Some(file) = rules.get(at) {
            let report = check(
                &std::fs::read_to_string(file).unwrap(),
                &card,
                &tokens,
                &names,
                &Caps::everything(),
            );
            println!("  verdict {:?}", report.verdict());
            for problem in &report.problems {
                println!("  problem: {problem:?}");
            }
            for clause in &report.clauses {
                println!("  [{}] {} => {:?}", clause.face, clause.text, clause.state);
            }
        }
    }
}

#[rstest]
#[case("Fast Forward")]
#[case("Weathered Sentinels")]
#[case("Paliano, the High City")]
#[case("Genju of the Realm")]
#[case("Cacophony Unleashed")]
#[case("Wolverine, Best There Is")]
#[case("Omnath, Locus of Mana")]
#[case("Hundred-Battle Veteran")]
#[case("Yuna, Grand Summoner")]
fn r5523_reads_back_whole(#[case] name: &str) {
    let report = r5523_report(name, &r5523_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::this_combat_not_this_turn("Fast Forward", r#""AttackedThisTurn""#, r#""Attacked""#)]
#[case::each_opponent("Fast Forward", r#""AttackedThisTurn""#, r#""Opponents""#)]
#[case::any_player_despite_defender(
    "Weathered Sentinels",
    r#""AttackLastTurnAttackersDespiteDefender""#,
    r#""AttackDespiteDefender""#
)]
#[case::any_color("Paliano, the High City", r#""DraftedColors""#, r#"{ "AnyColor": 1 }"#)]
#[case::without_the_draft_rule(
    "Paliano, the High City",
    r#"{ "Rule": { "rule": { "Draft": "ChooseColorsAround" } } },"#,
    ""
)]
#[case::genju_not_legendary("Genju of the Realm", r#"{ "AddSupertype": "Legendary" },"#, "")]
#[case::cacophony_lifelink("Cacophony Unleashed", r#""Deathtouch""#, r#""Lifelink""#)]
#[case::cacophony_any_permanent(
    "Cacophony Unleashed",
    r#"{ "Not": { "Type": "Enchantment" } },"#,
    ""
)]
#[case::wolverine_any_creature(
    "Wolverine, Best There Is",
    r#"{ "Type": "Creature" }, "Other""#,
    r#"{ "Type": "Creature" }"#
)]
#[case::omnath_red(
    "Omnath, Locus of Mana",
    r#""power": { "ManaInPoolOf": { "who": "You", "mana": "Green" } }"#,
    r#""power": { "ManaInPoolOf": { "who": "You", "mana": "Red" } }"#
)]
#[case::veteran_shield("Hundred-Battle Veteran", r#""finality""#, r#""shield""#)]
#[case::yuna_two_counters(
    "Yuna, Grand Summoner",
    r#""op": "Ge", "right": 1"#,
    r#""op": "Ge", "right": 2"#
)]
fn r5523_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r5523_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r5523_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
