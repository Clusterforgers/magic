//! Goal: every card the r55-22 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another fact of the turn, another count, another subject) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r5522_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r5522_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r5522_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r5522_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R5522_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r5522_probe() {
    if let Ok(json) = std::env::var("R5522_JSON") {
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
    if let Ok(list) = std::env::var("R5522_COMPILE") {
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
    let Ok(list) = std::env::var("R5522_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r5522_report(name, &r5522_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r5522_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Reidane, God of the Worthy // Valkmira, Protector's Shield")]
#[case("Storrev, Devkarin Lich")]
#[case("Spider-Man 2099")]
#[case("Yidaro, Wandering Monster")]
#[case("Diseased Vermin")]
#[case("Gollum, Obsessed Stalker")]
#[case("Glyph of Delusion")]
#[case("Churning Reservoir")]
#[case("Izzet Generatorium")]
#[case("Oblivious Bookworm")]
#[case("Mana Maze")]
#[case("Power Surge")]
#[case("Season of the Witch")]
fn r5522_reads_back_whole(#[case] name: &str) {
    let report = r5522_report(name, &r5522_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::valkmira_any_permanent(
    "Reidane, God of the Worthy // Valkmira, Protector's Shield",
    r#"["Other", { "ControlledBy": "You" }]"#,
    r#"[{ "ControlledBy": "You" }]"#
)]
#[case::storrev_this_turn(
    "Storrev, Devkarin Lich",
    r#""GravedThisCombat""#,
    r#""GravedThisTurn""#
)]
#[case::spider_man_from_exile("Spider-Man 2099", r#""PlayedNotFromHand""#, r#""PlayedFromExile""#)]
#[case::yidaro_cast(
    "Yidaro, Wandering Monster",
    r#""CycledSameNameThisGame""#,
    r#""CastSameNameThisGame""#
)]
#[case::yidaro_three("Yidaro, Wandering Monster", r#""right": 4"#, r#""right": 3"#)]
#[case::vermin_any_player("Diseased Vermin", r#""Opponents""#, r#""EachPlayer""#)]
#[case::gollum_any_damage("Gollum, Obsessed Stalker", r#""combat": true"#, r#""combat": false"#)]
#[case::gollum_times_gained("Gollum, Obsessed Stalker", r#""LifeGainedTotal""#, r#""LifeGained""#)]
#[case::glyph_toughness("Glyph of Delusion", r#""Power""#, r#""Toughness""#)]
#[case::glyph_end_step("Glyph of Delusion", r#""Upkeep""#, r#""End""#)]
#[case::reservoir_energy(
    "Churning Reservoir",
    r#""OilCountersRemovedUnder""#,
    r#""EnergyPaidOrLost""#
)]
#[case::reservoir_yours_died("Churning Reservoir", r#""EachPlayer""#, r#""You""#)]
#[case::generatorium_three("Izzet Generatorium", r#""right": 4"#, r#""right": 3"#)]
#[case::bookworm_face_down_twice(
    "Oblivious Bookworm",
    r#""TurnedFaceUp""#,
    r#""EnteredFaceDownUnder""#
)]
#[case::maze_opponents("Mana Maze", r#""EachPlayer""#, r#""Opponents""#)]
#[case::surge_lands_played("Power Surge", r#""UntappedLandsAsTurnBegan""#, r#""LandsPlayed""#)]
#[case::witch_couldnt_attack_ignored(
    "Season of the Witch",
    r#""CouldAttackThisTurn""#,
    r#""Untapped""#
)]
fn r5522_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r5522_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r5522_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
