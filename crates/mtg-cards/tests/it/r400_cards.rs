//! Goal: every card the r40-0 pack made fully automatic reads back as its printed text clause by clause, and a near miss
//! of each new wording (another selector, another count, another player) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r400_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r400_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r400_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r400_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states, the written variants and the compiler's reading of the cards named in `R400_PROBE`
/// (separated by `|`); a tool for writing rules files.
#[test]
fn r400_probe() {
    if let Ok(json) = std::env::var("R400_JSON") {
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
    let Ok(list) = std::env::var("R400_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r400_report(name, &r400_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r400_file(name)).unwrap();
        for face in &card.faces {
            for ability in &face.abilities {
                for text in mtg_cards::compile::written_variants(ability).iter().take(3) {
                    println!("  written: {text}");
                }
            }
        }
        let printed = common::printed()
            .into_iter()
            .find(|card| card.name == name)
            .unwrap();
        let compiled = mtg_cards::compile::compile(&printed);
        if let Some(card) = compiled.card {
            for face in &card.faces {
                for ability in &face.abilities {
                    println!("  compiled: {}", serde_json::to_string(ability).unwrap());
                }
            }
        }
    }
}

#[rstest]
#[case("Ekundu Cyclops")]
#[case("Magnetic Web")]
#[case("Land Grant")]
#[case("Kentaro, the Smiling Cat")]
#[case("Demon of Fate's Design")]
#[case("Hogaak, Arisen Necropolis")]
fn r400_reads_back_whole(#[case] name: &str) {
    let report = r400_report(name, &r400_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::another_creature(
    "Ekundu Cyclops",
    r#"{ "ControlledBy": "You" }"#,
    r#"{ "ControlledBy": "Opponent" }"#
)]
#[case::any_creature("Ekundu Cyclops", r#", { "ControlledBy": "You" }"#, "")]
#[case::all_creatures_bound(
    "Magnetic Web",
    r#""who": { "Each": [{ "Type": "Creature" }, { "HasCounter": { "Other": "magnet" } }] },"#,
    r#""who": { "Each": [{ "Type": "Creature" }] },"#
)]
#[case::blocks_until_your_next_turn(
    "Magnetic Web",
    r#""duration": "UntilEndOfTurn""#,
    r#""duration": "UntilEndOfYourNextTurn""#
)]
#[case::another_attacker(
    "Magnetic Web",
    r#""what": [{ "Type": "Creature" }, { "HasCounter": { "Other": "magnet" } }]"#,
    r#""what": [{ "Type": "Creature" }, { "HasCounter": { "Other": "charge" } }]"#
)]
#[case::any_cards_in_hand("Land Grant", r#"}, { "Type": "Land" }]"#, "}]")]
#[case::lands_in_hand("Land Grant", r#""op": "Eq""#, r#""op": "Ge""#)]
#[case::reveal_one_card(
    "Land Grant",
    r#""count": { "Count": [{ "In": "Hand" }, { "OwnedBy": "You" }] }"#,
    r#""count": 1"#
)]
#[case::ninja_spells("Kentaro, the Smiling Cat", r#""Samurai""#, r#""Ninja""#)]
#[case::its_mana_cost("Kentaro, the Smiling Cat", r#""OwnManaValue""#, r#""OwnManaCost""#)]
#[case::every_turn("Demon of Fate's Design", r#""once": true,"#, "")]
#[case::creature_spells(
    "Demon of Fate's Design",
    r#""what": [{ "Type": "Enchantment" }]"#,
    r#""what": [{ "Type": "Creature" }]"#
)]
#[case::copied("Hogaak, Arisen Necropolis", r#""CantSpendMana""#, r#""CantBeCopied""#)]
fn r400_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r400_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r400_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}

/// The compiler reads each new wording into the ability the card's file holds for it.
#[rstest]
#[case("Ekundu Cyclops", 0)]
#[case("Magnetic Web", 0)]
#[case("Magnetic Web", 1)]
#[case("Land Grant", 0)]
#[case("Kentaro, the Smiling Cat", 1)]
#[case("Demon of Fate's Design", 2)]
#[case("Hogaak, Arisen Necropolis", 0)]
fn r400_compiler_reads_the_wording(#[case] name: &str, #[case] ability: usize) {
    let card = common::printed()
        .into_iter()
        .find(|card| card.name == name)
        .unwrap();
    let compiled = mtg_cards::compile::compile(&card).card.unwrap();
    let file: mtg_cards::card::Card = serde_json::from_str(&r400_file(name)).unwrap();
    assert_eq!(
        compiled.faces[0].abilities[ability], file.faces[0].abilities[ability],
        "{name}"
    );
}
