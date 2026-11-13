//! Goal: every card the r55-12 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another type, another duration, another source of mana, another filter) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r5512_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r5512_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r5512_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r5512_names(&cards),
        &Caps::everything(),
    )
}

#[rstest]
#[case("Aura Graft")]
#[case("Enchantment Alteration")]
#[case("Soul Sculptor")]
#[case("Nivmagus Elemental")]
#[case("Open the Omenpaths")]
#[case("Aloy, Savior of Meridian")]
#[case("Thousand Moons Smithy // Barracks of the Thousand")]
#[case("The Everflowing Well // The Myriad Pools")]
#[case("Nathan Drake, Treasure Hunter")]
#[case("Synthesis Pod")]
#[case("Adarkar Unicorn")]
fn r5512_reads_back_whole(#[case] name: &str) {
    let report = r5512_report(name, &r5512_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::graft_keeps_control(
    "Aura Graft",
    r#""SetController": "You""#,
    r#""SetController": "Opponents""#
)]
#[case::graft_any_permanent(
    "Aura Graft",
    r#"{ "EnchantableBy": { "Target": 0 } }"#,
    r#"{ "Type": "Creature" }"#
)]
#[case::alteration_only_creatures(
    "Enchantment Alteration",
    r#""types": ["Creature", "Land"]"#,
    r#""types": ["Creature"]"#
)]
#[case::sculptor_until_end_of_turn(
    "Soul Sculptor",
    r#""UntilCreatureSpellCast""#,
    r#""UntilEndOfTurn""#
)]
#[case::sculptor_an_artifact(
    "Soul Sculptor",
    r#""SetTypes": ["Enchantment"]"#,
    r#""SetTypes": ["Artifact"]"#
)]
#[case::nivmagus_creature_spell(
    "Nivmagus Elemental",
    r#"{ "Type": "Instant" }"#,
    r#"{ "Type": "Creature" }"#
)]
#[case::nivmagus_three_counters("Nivmagus Elemental", r#""count": 2"#, r#""count": 3"#)]
#[case::omenpaths_artifacts("Open the Omenpaths", r#""Enchantment""#, r#""Artifact""#)]
#[case::aloy_toughness(
    "Aloy, Savior of Meridian",
    r#""stat": "Power""#,
    r#""stat": "Toughness""#
)]
#[case::barracks_any_mana(
    "Thousand Moons Smithy // Barracks of the Thousand",
    r#"{ "ManaFrom": "This" }"#,
    r#"{ "SpentFrom": { "from": {}, "least": 1 } }"#
)]
#[case::barracks_opponents_spells(
    "Thousand Moons Smithy // Barracks of the Thousand",
    r#""by": "You""#,
    r#""by": "Opponents""#
)]
#[case::nathan_owned_permanents(
    "Nathan Drake, Treasure Hunter",
    r#""abilities": [{ "ControlledBy": "You" }, { "Not": { "OwnedBy": "You" } }]"#,
    r#""abilities": [{ "ControlledBy": "You" }]"#
)]
#[case::nathan_any_number("Nathan Drake, Treasure Hunter", r#""max": 1"#, r#""max": 2"#)]
#[case::pod_two_plus("Synthesis Pod", "\"Sum\": [1,", "\"Sum\": [2,")]
#[case::unicorn_other_costs("Adarkar Unicorn", "\"CumulativeUpkeep\"", "\"OtherCosts\"")]
#[case::pools_forever(
    "The Everflowing Well // The Myriad Pools",
    r#""duration": "UntilEndOfTurn""#,
    r#""duration": "Forever""#
)]
fn r5512_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r5512_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r5512_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
