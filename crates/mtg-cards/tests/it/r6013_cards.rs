//! Goal: every clause the r60-13 pack made automatic reads back as its printed text, the cards made whole are accepted
//! whole, and a near miss (another number, another player, another zone, another condition) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{ClauseState, Report, Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r6013_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r6013_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r6013_report(name: &str, text: &str) -> Report {
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
        &r6013_names(&cards),
        &Caps::everything(),
    )
}

/// Whether the clause that starts with `start` reads back.
fn r6013_reads(report: &Report, start: &str) -> bool {
    report
        .clauses
        .iter()
        .find(|clause| clause.text.starts_with(start))
        .is_some_and(|clause| matches!(clause.state, ClauseState::Reads { .. }))
}

#[rstest]
#[case("Argentum Masticore")]
#[case("Aven Mindcensor")]
#[case("Wolverine, Fierce Fighter")]
#[case("Freyalise's Winds")]
#[case("Mana Clash")]
#[case("Valentin, Dean of the Vein // Lisette, Dean of the Root")]
#[case("Ajani, Nacatl Pariah // Ajani, Nacatl Avenger")]
fn r6013_reads_back_whole(#[case] name: &str) {
    let report = r6013_report(name, &r6013_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case("Argentum Masticore", "At the beginning of your upkeep")]
#[case("Grist, the Hunger Tide", "+1:")]
#[case("Primeval Spawn", "If this creature would enter")]
#[case("Enduring Angel // Angelic Enforcer", "Whenever this creature attacks")]
fn r6013_clause_reads_back(#[case] name: &str, #[case] start: &str) {
    let report = r6013_report(name, &r6013_file(name));
    assert!(r6013_reads(&report, start), "{name}: {:#?}", report.clauses);
}

#[rstest]
#[case::masticore_greater_value(
    "Argentum Masticore",
    "At the beginning of your upkeep",
    r#""op": "Le""#,
    r#""op": "Ge""#
)]
#[case::masticore_your_permanent(
    "Argentum Masticore",
    "At the beginning of your upkeep",
    r#""ControlledBy": "Opponents""#,
    r#""ControlledBy": "You""#
)]
#[case::grist_a_zombie(
    "Grist, the Hunger Tide",
    "+1:",
    r#""Subtype": "Insect""#,
    r#""Subtype": "Zombie""#
)]
#[case::grist_two_loyalty(
    "Grist, the Hunger Tide",
    "+1:",
    r#""counter": "loyalty""#,
    r#""counter": "loyalty", "count": 2"#
)]
#[case::spawn_to_graveyard(
    "Primeval Spawn",
    "If this creature would enter",
    r#""Exile": { "what": "Triggering" }"#,
    r#""Destroy": { "what": "Triggering" }"#
)]
#[case::spawn_one_mana(
    "Primeval Spawn",
    "If this creature would enter",
    r#""right": 0"#,
    r#""right": 1"#
)]
#[case::mindcensor_three_cards(
    "Aven Mindcensor",
    "If an opponent",
    r#""count": 4"#,
    r#""count": 3"#
)]
#[case::mindcensor_every_player(
    "Aven Mindcensor",
    "If an opponent",
    r#""Opponents""#,
    r#""EachPlayer""#
)]
#[case::wolverine_prevented(
    "Wolverine, Fierce Fighter",
    "If damage would",
    r#""HealFirst""#,
    r#""Prevent""#
)]
#[case::wolverine_any_recipient(
    "Wolverine, Fierce Fighter",
    "If damage would",
    r#""to": "This""#,
    r#""to": "Attached""#
)]
#[case::winds_ice_counters(
    "Freyalise's Winds",
    "If a permanent",
    r#""UntapRemovesCounters": { "Other": "wind""#,
    r#""UntapRemovesCounters": { "Other": "ice""#
)]
#[case::clash_two_damage("Mana Clash", "You and target", r#""amount": 1"#, r#""amount": 2"#)]
#[case::valentin_your_creatures(
    "Valentin, Dean of the Vein // Lisette, Dean of the Root",
    "If a nontoken",
    r#""ControlledBy": "Opponents""#,
    r#""ControlledBy": "You""#
)]
#[case::ajani_keeps_lands(
    "Ajani, Nacatl Pariah // Ajani, Nacatl Avenger",
    "\u{2212}4",
    r#""Type": "Planeswalker""#,
    r#""Type": "Battle""#
)]
fn r6013_a_near_miss_does_not_read_back(
    #[case] name: &str,
    #[case] start: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let file = r6013_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r6013_report(name, &file.replacen(from, to, 1));
    assert!(!r6013_reads(&report, start), "{name}: {to}");
}
