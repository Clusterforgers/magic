//! Goal: every card the r120-01 pack made fully automatic reads back as its printed text clause by clause, and a near miss
//! of each (another number or counter kind) does not.
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
#[case("Chira, All In")]
#[case("La Forge, Thoughtful as Ever")]
#[case("Matoc, Lavamancer")]
#[case("U.S.S. Enterprise-D, Galaxy-Class")]
#[case("Yume, Chronicler of Valor")]
fn r120_01_reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &compact(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
// The trigger is the first ability, so its counter kind is the first one named.
#[case::another_kind(
    "U.S.S. Enterprise-D, Galaxy-Class",
    r#""counter":"charge""#,
    r#""counter":"+1/+1""#
)]
#[case::an_opponent("Chira, All In", r#""who":"You""#, r#""who":"Opponents""#)]
#[case::an_opponent_put_it("La Forge, Thoughtful as Ever", r#""by":"You""#, r#""by":"Opponents""#)]
#[case::exactly_three("Matoc, Lavamancer", r#""amount":2"#, r#""amount":3"#)]
#[case::the_second_time("U.S.S. Enterprise-D, Galaxy-Class", r#""nth":1"#, r#""nth":2"#)]
fn r120_01_a_near_miss_does_not_read_back(
    #[case] name: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let text = compact(name);
    assert!(text.contains(from), "{name} holds {from}");
    let report = report_of(name, &text.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
