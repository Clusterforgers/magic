//! Goal: every card the r55-11 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another cost, count, zone or keyword) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r5511_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r5511_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r5511_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r5511_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R5511_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r5511_probe() {
    if let Ok(json) = std::env::var("R5511_JSON") {
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
    if let Ok(list) = std::env::var("R5511_COMPILE") {
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
    let Ok(list) = std::env::var("R5511_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r5511_report(name, &r5511_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r5511_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Nethergoyf")]
#[case("Soulflayer")]
#[case("Duplicant")]
#[case("Illusionary Terrain")]
#[case("Omnath, Locus of All")]
#[case("Elemental Resonance")]
#[case("Volcano Hellion")]
#[case("Yuriko, the Tiger's Shadow")]
#[case("Pulse of Llanowar")]
#[case("Harvest Mage")]
#[case("Kalain, Reclusive Painter")]
#[case("Coin of Mastery")]
#[case("Thermal Flux")]
#[case("Mr. House, President and CEO")]
fn r5511_reads_back_whole(#[case] name: &str) {
    let report = r5511_report(name, &r5511_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::yuriko_plain_ninjutsu(
    "Yuriko, the Tiger's Shadow",
    r#""CommanderNinjutsu""#,
    r#""Ninjutsu""#
)]
#[case::yuriko_you_lose("Yuriko, the Tiger's Shadow", r#""who": "Opponents", "#, "")]
#[case::pulse_one_mana("Pulse of Llanowar", r#""OnlyChosenColor""#, r#""OneChosenColor""#)]
#[case::pulse_any_land("Pulse of Llanowar", r#"{ "Supertype": "Basic" }, "#, "")]
#[case::mage_same_amount("Harvest Mage", r#""OneChosenColor""#, r#""OnlyChosenColor""#)]
#[case::mage_for_ever("Harvest Mage", r#", "duration": "UntilEndOfTurn""#, "")]
#[case::kalain_artifacts(
    "Kalain, Reclusive Painter",
    r#""TreasureManaSpent""#,
    r#""ArtifactManaSpent""#
)]
#[case::kalain_itself_too("Kalain, Reclusive Painter", r#""Other", "#, "")]
#[case::coin_treasures("Coin of Mastery", r#""ArtifactManaSpent""#, r#""TreasureManaSpent""#)]
#[case::flux_for_ever("Thermal Flux", r#""UntilEndOfTurn""#, r#""Forever""#)]
#[case::flux_basic(
    "Thermal Flux",
    r#"{ "Supertype": "Snow" }"#,
    r#"{ "Supertype": "Basic" }"#
)]
#[case::house_five_or_higher("Mr. House, President and CEO", r#""right": 4"#, r#""right": 5"#)]
#[case::house_artifacts("Mr. House, President and CEO", r#""Treasure""#, r#""Clue""#)]
#[case::hellion_opponents_life(
    "Volcano Hellion",
    r#"{ "Life": "You" }"#,
    r#"{ "Life": "Opponents" }"#
)]
#[case::hellion_preventable("Volcano Hellion", r#", "unpreventable": true"#, "")]
#[case::resonance_your_upkeep("Elemental Resonance", r#""Main1""#, r#""Upkeep""#)]
#[case::resonance_its_own_cost("Elemental Resonance", r#""Attached""#, r#""This""#)]
#[case::omnath_two_symbols("Omnath, Locus of All", r#""right": 3"#, r#""right": 2"#)]
#[case::omnath_mana_value("Omnath, Locus of All", r#""ColoredManaSymbols""#, r#""ManaValue""#)]
#[case::duplicant_not_a_shapeshifter(
    "Duplicant",
    r#"{ "AddSubtype": "Shapeshifter" }"#,
    r#"{ "AddSubtype": "Horror" }"#
)]
#[case::duplicant_any_card(
    "Duplicant",
    r#"{ "ExiledWith": "This" }, { "Type": "Creature" }"#,
    r#"{ "ExiledWith": "This" }"#
)]
#[case::terrain_second_first(
    "Illusionary Terrain",
    r#""SetLandTypesNthChosen": 1"#,
    r#""SetLandTypesNthChosen": 0"#
)]
#[case::terrain_any_land("Illusionary Terrain", r#"{ "Supertype": "Basic" }, "#, "")]
#[case::soulflayer_without_reach("Soulflayer", r#""Reach", "#, "")]
#[case::soulflayer_any_card("Soulflayer", r#", { "Type": "Creature" }"#, "")]
#[case::goyf_colors_count(
    "Nethergoyf",
    r#"{ "ExileCardTypes": 4 }"#,
    r#"{ "ExileCardTypes": 3 }"#
)]
#[case::goyf_five_cards(
    "Nethergoyf",
    r#"{ "ExileCardTypes": 4 }"#,
    r#"{ "ExileFromGraveyard": { "filter": ["Other"], "count": 4 } }"#
)]
fn r5511_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r5511_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r5511_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}

#[test]
fn r5511_cyclopean_tomb_reads_back_its_first_ability_and_leaves_the_second_by_hand() {
    let name = "Cyclopean Tomb";
    let report = r5511_report(name, &r5511_file(name));
    let first = &report.clauses[0];
    assert!(
        matches!(first.state, mtg_cards::proposal::ClauseState::Reads { .. }),
        "{first:#?}"
    );
    let file = r5511_file(name);
    let missed = r5511_report(name, &file.replacen(r#""Upkeep""#, r#""Draw""#, 1));
    assert!(!matches!(
        missed.clauses[0].state,
        mtg_cards::proposal::ClauseState::Reads { .. }
    ));
}
