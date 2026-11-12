//! Goal: every card the r30-0c pack made fully automatic reads back as its printed text clause by clause, two activated
//! abilities that differ only in cost read as one line with either cost, and a near miss (another number, kind, cap,
//! cost or effect) never reads back.
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
        .chain(common::draft_names())
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

/// Prints the clause states of the cards named in `R300C_PROBE` (separated by `|`), read from the directory
/// `R300C_DIR` when set; a tool for writing rules files without rebuilding the crate.
#[test]
fn probe() {
    let Ok(list) = std::env::var("R300C_PROBE") else {
        return;
    };
    let dir = std::env::var("R300C_DIR").map_or_else(|_| common::cards_dir(), Into::into);
    for name in list.split('|') {
        let path = dir.join(format!("{}.json", slug(name)));
        let report = report_of(name, &std::fs::read_to_string(&path).unwrap());
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
        if std::env::var("R300C_WORDS").is_ok() {
            let card: mtg_cards::card::Card =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let listed: Vec<_> = card
                .abilities()
                .map(|(_, ability)| ability.clone())
                .collect();
            for (at, ability) in listed.iter().enumerate() {
                println!(
                    "  words {at}: {:?}",
                    mtg_cards::compile::written_variants(ability)
                );
                if let Some(next) = listed.get(at + 1) {
                    println!(
                        "  pair {at}: {:?}",
                        mtg_cards::compile::either_way_pair_texts(ability, next)
                    );
                }
            }
        }
    }
}

#[rstest]
#[case("Pearl Shard")]
#[case("Skeleton Shard")]
#[case("Crystal Shard")]
#[case("Heartwood Shard")]
#[case("Granite Shard")]
#[case("Clockwork Avian")]
#[case("Clockwork Beast")]
#[case("Clockwork Steed")]
#[case("Clockwork Swarm")]
#[case("Rhys, the Evermore")]
#[case("Behemoth's Herald")]
#[case("Sphinx's Herald")]
#[case("Scrying Sheets")]
#[case("Urza's Avenger")]
#[case("Jetmir's Fixer")]
#[case("Paragon of Modernity")]
#[case("Helm of Possession")]
#[case("Rubinia Soulsinger")]
#[case("Willow Satyr")]
#[case("Hivis of the Scale")]
#[case("Marshland Bloodcaster")]
#[case("Tam, the Possibility")]
#[case("Orcish Librarian")]
#[case("Moonsilver Key")]
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
#[case::same_cost_twice("Pearl Shard", r#""{W}""#, r#""{3}""#)]
#[case::other_amount("Pearl Shard", r#""PreventUpTo":2"#, r#""PreventUpTo":3"#)]
#[case::other_keyword(
    "Heartwood Shard",
    r#""AddKeyword":"Trample""#,
    r#""AddKeyword":"Flying""#
)]
#[case::other_damage("Granite Shard", r#""amount":1"#, r#""amount":2"#)]
#[case::other_cap("Clockwork Avian", "[4,", "[5,")]
#[case::other_kind(
    "Clockwork Beast",
    r#""count":{"ChosenAs":"n"},"counter":{"Other":"+1/+0"}"#,
    r#""count":{"ChosenAs":"n"},"counter":"+1/+1""#
)]
#[case::exactly_x("Clockwork Steed", "[0,", "[1,")]
#[case::any_time(
    "Clockwork Swarm",
    r#""condition":{"All":["YourTurn",{"Step":"Upkeep"}]},"#,
    ""
)]
#[case::up_to_three("Rhys, the Evermore", r#"{"AnyCounters":{"Target":0}}"#, "3")]
#[case::all_of_them("Rhys, the Evermore", r#","up_to":true"#, "")]
#[case::other_name("Behemoth's Herald", r#""Color":"Red""#, r#""Color":"Blue""#)]
#[case::any_card("Scrying Sheets", r#",{"Supertype":"Snow"}"#, "")]
#[case::three_keywords("Urza's Avenger", r#""Banding""#, r#""Trample""#)]
#[case::other_pump("Urza's Avenger", r#""power":-1"#, r#""power":1"#)]
#[case::two_treasures("Jetmir's Fixer", r#""right":1"#, r#""right":2"#)]
#[case::four_colors("Paragon of Modernity", r#""right":3"#, r#""right":4"#)]
#[case::while_tapped_only(
    "Helm of Possession",
    r#""WhileYouControlSourceTapped""#,
    r#""WhileSourceTapped""#
)]
#[case::while_controlled_only(
    "Rubinia Soulsinger",
    r#""WhileYouControlSourceTapped""#,
    r#""WhileYouControlSource""#
)]
#[case::once("Tam, the Possibility", r#""stat":"Subtypes""#, r#""stat":"Name""#)]
#[case::half_life(
    "Marshland Bloodcaster",
    r#"{"Stat":{"of":"This","stat":"ManaValue"}}"#,
    r#"{"Half":{"of":{"Stat":{"of":"This","stat":"ManaValue"}},"up":true}}"#
)]
#[case::from_any_graveyard("Skeleton Shard", r#",{"OwnedBy":"You"}"#, "")]
#[case::any_artifact(
    "Moonsilver Key",
    r#"{"Not":{"HasAbility":"Mana"}}"#,
    r#"{"Not":{"HasAbility":"Any"}}"#
)]
#[case::other_milled("Tazri, Stalwart Survivor", r#""NonManaActivated""#, r#""Mana""#)]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}

/// Tazri's activated ability reads back and only the granted mana ability is left to the table.
#[test]
fn tazri_keeps_only_its_grant_by_hand() {
    let report = report("Tazri, Stalwart Survivor");
    let states: Vec<String> = report
        .clauses
        .iter()
        .map(|clause| format!("{:?}", clause.state))
        .collect();
    assert_eq!(states.len(), 2, "{states:?}");
    assert!(states[0].starts_with("Manual"), "{states:?}");
    assert!(states[1].starts_with("Reads"), "{states:?}");
}
