//! Goal: every card the r109-05 pack made fully automatic reads back as its printed text clause by clause, and a near miss
//! of each (another quirk, recipient, assigner, keyword or extra-turn rider) does not.
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

/// The rules file of a corpus card as compact JSON.
fn compact(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    let text = std::fs::read_to_string(&path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    serde_json::to_string(&value).unwrap()
}

#[rstest]
#[case("Thrasta, Tempest's Roar")]
#[case("Butcher Orgg")]
#[case("Cunning Giant")]
#[case("Ogre Enforcer")]
#[case("Defensive Formation")]
#[case("Alchemist's Gambit")]
#[case("Bumi, Unleashed")]
#[case("Illusionist's Gambit")]
#[case("Portal Manipulator")]
fn r109_05_reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &compact(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::plain_trample(
    "Thrasta, Tempest's Roar",
    r#""TrampleOverPlaneswalkers""#,
    r#""Trample""#
)]
#[case::giant_quirk("Butcher Orgg", "R109AssignsAmongDefenders", "R109UnblockedToCreature")]
#[case::orgg_quirk(
    "Cunning Giant",
    "R109UnblockedToCreature",
    "R109AssignsAmongDefenders"
)]
#[case::as_unblocked("Cunning Giant", "R109UnblockedToCreature", "AssignsAsUnblocked")]
#[case::no_damage("Ogre Enforcer", "R109LethalFromOneSource", "AssignsNoCombatDamage")]
#[case::any_attacker(
    "Defensive Formation",
    r#",{"AttackingPlayer":"You"}"#,
    r#","Attacking""#
)]
#[case::opponents_assign(
    "Defensive Formation",
    r#"{"AttackingPlayer":"You"}"#,
    r#"{"AttackingPlayer":"Opponents"}"#
)]
#[case::preventable(
    "Alchemist's Gambit",
    r#""r109_unpreventable":true,"at_end""#,
    r#""at_end""#
)]
#[case::bumi_all_turn("Bumi, Unleashed", r#"{"R109ThatCombat":0}"#, r#""UntilEndOfTurn""#)]
#[case::bumi_lands_attack("Bumi, Unleashed", r#"{"Not":{"Type":"Land"}}"#, r#"{"Type":"Land"}"#)]
#[case::gambit_owner(
    "Illusionist's Gambit",
    r#""CantAttackYou":"Both""#,
    r#""CantAttackYou":"Player""#
)]
#[case::gambit_no_untap("Illusionist's Gambit", r#"{"Untap":{"what":{"Bound":"those"}}},"#, "")]
#[case::portal_any_controller(
    "Portal Manipulator",
    r#",{"ControlledBy":{"OpponentsOf":{"Target":0}}}"#,
    ""
)]
#[case::portal_choice("Portal Manipulator", r#","r109_to":{"Target":0}"#, "")]
#[case::no_loss("Alchemist's Gambit", r#","at_end":[{"LoseGame":{}}]"#, "")]
fn r109_05_a_near_miss_does_not_read_back(
    #[case] name: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let text = compact(name);
    assert!(text.contains(from), "{name} holds {from}");
    let report = report_of(name, &text.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
