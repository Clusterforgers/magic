//! Goal: every card the p165 pack made fully automatic reads back as its printed text clause by clause through
//! `check_exact`, and a near miss does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Report, Verdict, check_exact};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn p165_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn p165_report(name: &str, text: &str) -> Report {
    let cards = common::printed();
    let names: BTreeSet<String> = cards
        .iter()
        .flat_map(|card: &PrintedCard| {
            std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name))
        })
        .map(|name| name.to_lowercase())
        .collect();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards.iter().find(|card| card.name == name).unwrap();
    check_exact(text, card, &tokens, &names, &Caps::everything())
}

fn p165_verdict(name: &str, text: &str) -> Verdict {
    p165_report(name, text).verdict()
}

#[rstest]
#[case("Tawnos's Coffin")]
#[case("Takklemaggot")]
#[case("Psychic Battle")]
#[case("Miss Highwater")]
fn p165_reads_back_whole(#[case] name: &str) {
    let report = p165_report(name, &p165_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?} {:#?}",
        report.problems,
        report.clauses
    );
}

#[rstest]
#[case::coffin_untapped(
    "Tawnos's Coffin",
    r#"{ "Untapped": { "what": ["This"] } }"#,
    r#"{ "Tapped": { "what": ["This"] } }"#
)]
#[case::coffin_no_note(
    "Tawnos's Coffin",
    r#""R122BattlefieldNoted": { "noted": "noted" }"#,
    r#""Battlefield": {}"#
)]
#[case::coffin_no_auras(
    "Tawnos's Coffin",
    r#"{ "Subtype": "Aura" }"#,
    r#"{ "Subtype": "Equipment" }"#
)]
#[case::maggot_own_control("Takklemaggot", r#""under": "You""#, r#""under": "Opponents""#)]
#[case::maggot_still_an_aura("Takklemaggot", r#"{ "R2051RemoveSubtype": "Aura" },"#, "")]
#[case::maggot_two_damage("Takklemaggot", r#""amount": 1"#, r#""amount": 2"#)]
#[case::battle_loud("Psychic Battle", r#"{ "quiet_own_name": true }"#, "{}")]
#[case::battle_you_change(
    "Psychic Battle",
    r#""r115_who": { "Bound": "winner" }"#,
    r#""r115_who": "You""#
)]
#[case::highwater_any_player("Miss Highwater", r#""op": "Eq""#, r#""op": "Ge""#)]
#[case::highwater_for_good("Miss Highwater", r#""until""#, r#""this_turn": true, "until""#)]
#[case::highwater_creatures_only(
    "Miss Highwater",
    r#"{ "AnyOf": [{ "Type": "Artifact" }, { "Type": "Creature" }] }"#,
    r#"{ "Type": "Creature" }"#
)]
fn p165_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = p165_file(name);
    assert!(file.contains(from), "{name}: the file has {from}");
    assert_ne!(
        p165_verdict(name, &file.replacen(from, to, 1)),
        Verdict::Accepted,
        "{name}: {from} -> {to}"
    );
}
