//! Goal: every card of the p162 pack (face-down piles and exiles, delay counters, lands of others tapped for mana, a land entering then a sacrifice, exact 1 damage or life loss) reads back as its printed text and the checker proves its rules file whole, so it
//! is `Verified`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;

use mtg_cards::load::slug;
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::Caps;
use rstest::rstest;

#[rstest]
#[case("Parallel Thoughts")]
#[case("Piracy")]
#[case("Land Equilibrium")]
#[case("Ertai's Meddling")]
#[case("Grimoire Thief")]
#[case("Vulcan Mind-Meld")]
#[case("Sahir, Visitor in Darkness")]
fn p162_card_is_verified(#[case] name: &str) {
    let library = common::Corpus::repo().library();
    let printed = common::printed();
    let names: BTreeSet<String> = printed
        .iter()
        .map(|card| card.name.to_lowercase())
        .collect();
    let card = printed
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("no fixture for {name}"));
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    let text = std::fs::read_to_string(&path).unwrap();
    let report = check_exact(&text, card, &library, &names, &Caps::everything());
    assert!(
        report.verdict() == Verdict::Accepted && report.summary() == "ACCEPTED Verified",
        "{}",
        report.render(true)
    );
}
