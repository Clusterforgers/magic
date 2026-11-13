//! Goal: every card the r55-14 pack made automatic reads back as its printed text clause by clause, a near miss of each
//! new piece does not, and the validator refuses each new node outside the shapes the engine runs.
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

fn report_of(name: &str, text: &str) -> mtg_cards::proposal::Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    check(text, card, &tokens, &names(&cards), &Caps::everything())
}

fn file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

/// A rules file of the corpus with one piece of its compact JSON replaced.
fn altered(name: &str, from: &str, to: &str) -> String {
    let compact: serde_json::Value = serde_json::from_str(&file(name)).unwrap();
    let compact = serde_json::to_string(&compact).unwrap();
    assert!(compact.contains(from), "{name} holds {from}");
    compact.replacen(from, to, 1)
}

#[rstest]
#[case("Fatal Lore")]
#[case("Time and Tide")]
#[case("Teferi's Reproach")]
#[case("Thran Portal")]
#[case("Behemoth of Vault 0")]
#[case("Spectral Adversary")]
fn reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::the_controller_chooses("Fatal Lore", r#""who":"Opponents""#, r#""who":"You""#)]
#[case::the_caster_draws(
    "Fatal Lore",
    r#""count":{"ChosenAs":"drawn"},"who":"ModesChooser""#,
    r#""count":{"ChosenAs":"drawn"},"who":"You""#
)]
#[case::creatures_of_the_caster(
    "Fatal Lore",
    r#"{"ControlledBy":"ModesChooser"}"#,
    r#"{"ControlledBy":"You"}"#
)]
#[case::up_to_three_targets("Fatal Lore", r#""max":2"#, r#""max":3"#)]
#[case::regenerable("Fatal Lore", r#""regenerate":false"#, r#""regenerate":true"#)]
#[case::until_your_next_turn(
    "Teferi's Reproach",
    r#""duration":"UntilTheirNextTurn""#,
    r#""duration":"UntilYourNextTurn""#
)]
#[case::lands_too("Teferi's Reproach", r#",{"Not":{"Type":"Land"}}"#, "")]
#[case::any_player("Teferi's Reproach", r#""what":"Opponent""#, r#""what":"Player""#)]
#[case::other_targets(
    "Behemoth of Vault 0",
    r#"{"Not":{"Type":"Land"}}"#,
    r#"{"Type":"Creature"}"#
)]
#[case::exile_instead(
    "Behemoth of Vault 0",
    r#"{"Destroy":{"what":{"Bound":"paid_for"}}}"#,
    r#"{"Exile":{"what":{"Bound":"paid_for"}}}"#
)]
#[case::toughness_in_energy(
    "Behemoth of Vault 0",
    r#""stat":"ManaValue""#,
    r#""stat":"Toughness""#
)]
#[case::two_life("Thran Portal", r#""life":1"#, r#""life":2"#)]
#[case::other_lands(
    "Thran Portal",
    r#""applies_to":["This"]"#,
    r#""applies_to":[{"Type":"Land"}]"#
)]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {to}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::their_turn_on_a_continuous_effect(
    "Teferi's Reproach",
    r#"{"PhaseOut":{"what""#,
    r#"{"Continuous":{"affected":{"Target":0},"mods":[],"duration":"UntilTheirNextTurn"}},{"PhaseOut":{"what""#
)]
#[case::their_turn_on_another_rule(
    "Teferi's Reproach",
    r#"{"LifeTotalCantChange":{"Target":0}}"#,
    r#"{"CantDraw":{"Target":0}}"#
)]
#[case::life_on_equip("Thran Portal", r#""activating":"Mana""#, r#""activating":"Equip""#)]
#[case::life_with_mana("Thran Portal", r#""mana":"{0}""#, r#""mana":"{1}""#)]
#[case::life_reduced("Thran Portal", r#""change":"Increase""#, r#""change":"Reduce""#)]
fn the_validator_refuses_other_shapes(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_eq!(
        report.verdict(),
        Verdict::Invalid,
        "{name}: {to}: {:#?}",
        report.problems
    );
}
