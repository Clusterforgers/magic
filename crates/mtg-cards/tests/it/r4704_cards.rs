//! Goal: every card the r47-04 pack made automatic reads back as its printed text clause by clause, and a near miss
//! of each new wording (another count, another selector, another player) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r4704_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r4704_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap_or_else(|_| "{\"faces\":[]}".into())
}

fn r4704_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r4704_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the compiler's reading, the clause states and the written variants of the cards named in `R4704_PROBE`
/// (separated by `|`), and the written variants of the abilities in `R4704_JSON` (one per line); a tool for writing
/// rules files.
#[test]
fn r4704_probe() {
    if let Ok(json) = std::env::var("R4704_JSON") {
        for line in json.split('\n').filter(|l| !l.trim().is_empty()) {
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
    let Ok(list) = std::env::var("R4704_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let printed = common::printed()
            .into_iter()
            .find(|card| card.name == name)
            .unwrap_or_else(|| panic!("no fixture for {name}"));
        let compiled = mtg_cards::compile::compile(&printed);
        println!("== {name}");
        for clause in &compiled.clauses {
            println!(
                "  clause read={} {:?}: {}",
                clause.read, clause.kind, clause.text
            );
        }
        if let Some(card) = compiled.card {
            for face in &card.faces {
                for ability in &face.abilities {
                    println!("  compiled: {}", serde_json::to_string(ability).unwrap());
                }
            }
        }
        let file = r4704_file(name);
        if file.contains("oracle_id") {
            let report = r4704_report(name, &file);
            println!("  file verdict: {:?}", report.verdict());
            for problem in &report.problems {
                println!("  problem: {problem:?}");
            }
            for clause in &report.clauses {
                println!("  [{}] {} => {:?}", clause.face, clause.text, clause.state);
            }
            for stray in &report.strays {
                println!("  stray: {stray:?}");
            }
            let card: mtg_cards::card::Card = serde_json::from_str(&file).unwrap();
            for face in &card.faces {
                for ability in &face.abilities {
                    for text in mtg_cards::compile::written_variants(ability).iter().take(3) {
                        println!("  written: {text}");
                    }
                }
            }
        }
    }
}

#[rstest]
#[case("Overclocked Electromancer")]
#[case("Ghastly Conscription")]
#[case("Become Anonymous")]
#[case("Jeskai Infiltrator")]
#[case("Triumph of Saint Katherine")]
#[case("Slaughter the Strong")]
#[case("Destined Confrontation")]
#[case("Darksteel Garrison")]
fn r4704_reads_back_whole(#[case] name: &str) {
    let report = r4704_report(name, &r4704_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

/// The printed clause of a card that one of its rules reads back as.
fn r4704_written_by(name: &str, clause: &str) -> bool {
    let card: mtg_cards::card::Card = serde_json::from_str(&r4704_file(name)).unwrap();
    card.faces
        .iter()
        .flat_map(|face| &face.abilities)
        .any(|ability| {
            mtg_cards::compile::written_variants(ability)
                .iter()
                .any(|written| mtg_cards::compile::same_text(written, clause))
        })
}

#[rstest]
#[case::excess(
    "Overclocked Electromancer",
    "Whenever this creature deals combat damage to a creature, if that creature was dealt excess damage this turn, you get X {E}, where X is that excess damage."
)]
#[case::pile_manifest(
    "Ghastly Conscription",
    "Exile all creature cards from target player's graveyard in a face-down pile, shuffle that pile, then manifest those cards."
)]
#[case::pile_cloak_tapped(
    "Become Anonymous",
    "Exile target nontoken creature you own and the top two cards of your library in a face-down pile, shuffle that pile, then cloak those cards. They enter tapped."
)]
#[case::pile_back(
    "Triumph of Saint Katherine",
    "When this creature is put into your graveyard from the battlefield, exile it and the top six cards of your library in a face-down pile. If you do, shuffle that pile and put it back on top of your library."
)]
#[case::keep_power(
    "Slaughter the Strong",
    "Each player chooses any number of creatures they control with total power 4 or less, then sacrifices all other creatures they control."
)]
#[case::fortified("Darksteel Garrison", "Fortified land has indestructible.")]
fn r4704_the_printed_clause_is_written(#[case] name: &str, #[case] clause: &str) {
    assert!(r4704_written_by(name, clause), "{name}: {clause}");
}

#[rstest]
#[case::excess_of_another_damage(
    "Overclocked Electromancer",
    "Whenever this creature deals combat damage to a creature, if that creature was dealt excess damage this turn, you get X {E}, where X is that damage."
)]
#[case::excess_to_a_player(
    "Overclocked Electromancer",
    "Whenever this creature deals combat damage to a player, if that creature was dealt excess damage this turn, you get X {E}, where X is that excess damage."
)]
#[case::pile_cloaked_instead(
    "Ghastly Conscription",
    "Exile all creature cards from target player's graveyard in a face-down pile, shuffle that pile, then cloak those cards."
)]
#[case::pile_of_another_kind(
    "Ghastly Conscription",
    "Exile all artifact cards from target player's graveyard in a face-down pile, shuffle that pile, then manifest those cards."
)]
#[case::pile_untapped(
    "Become Anonymous",
    "Exile target nontoken creature you own and the top two cards of your library in a face-down pile, shuffle that pile, then cloak those cards."
)]
#[case::pile_of_three(
    "Become Anonymous",
    "Exile target nontoken creature you own and the top three cards of your library in a face-down pile, shuffle that pile, then cloak those cards. They enter tapped."
)]
#[case::pile_another_count(
    "Jeskai Infiltrator",
    "When this creature deals combat damage to a player, exile it and the top two cards of your library in a face-down pile, shuffle that pile, then manifest those cards."
)]
#[case::pile_back_five(
    "Triumph of Saint Katherine",
    "When this creature is put into your graveyard from the battlefield, exile it and the top five cards of your library in a face-down pile. If you do, shuffle that pile and put it back on top of your library."
)]
#[case::pile_back_any_graveyard(
    "Triumph of Saint Katherine",
    "When this creature is put into a graveyard from the battlefield, exile it and the top six cards of your library in a face-down pile. If you do, shuffle that pile and put it back on top of your library."
)]
#[case::keep_power_five(
    "Slaughter the Strong",
    "Each player chooses any number of creatures they control with total power 5 or less, then sacrifices all other creatures they control."
)]
#[case::keep_power_artifacts(
    "Slaughter the Strong",
    "Each player chooses any number of artifacts they control with total power 4 or less, then sacrifices all other artifacts they control."
)]
#[case::fortified_creature("Darksteel Garrison", "Fortified creature has indestructible.")]
fn r4704_a_near_miss_is_not_written(#[case] name: &str, #[case] clause: &str) {
    assert!(!r4704_written_by(name, clause), "{name}: {clause}");
}
