//! Goal: every card of card pack r37-6 is Verified from its corpus file, and the checker reads each of its rules back as
//! the printed text (`Accepted`, no clause by hand, unwritten or differing).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use rstest::rstest;

use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::{Caps, Support, support};

use crate::common;

#[rstest]
#[case::aeon_engine("Aeon Engine")]
#[case::hama_pashar_ruin_seeker("Hama Pashar, Ruin Seeker")]
#[case::pramikon_sky_rampart("Pramikon, Sky Rampart")]
#[case::teyo_geometric_tactician("Teyo, Geometric Tactician")]
#[case::pramikon_sky_rampart("Pramikon, Sky Rampart")]
#[case::teyo_geometric_tactician("Teyo, Geometric Tactician")]
#[case::cryptic_gateway("Cryptic Gateway")]
#[case::bucknards_everfull_purse("Bucknard's Everfull Purse")]
#[case::cirdan_the_shipwright("Círdan the Shipwright")]
#[case::hithlain_rope("Hithlain Rope")]
#[case::menacing_ogre("Menacing Ogre")]
#[case::mob_verdict("Mob Verdict")]
#[case::prisoners_dilemma("Prisoner's Dilemma")]
fn the_card_is_verified_and_reads_back_as_printed(#[case] name: &str) {
    let printed = common::printed()
        .into_iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("no fixture for {name}"));
    let library = common::Corpus::repo().library();
    let card = library.card(&printed.oracle_id).unwrap();
    assert_eq!(
        support(Some(card), &printed, &Caps::everything()).support,
        Support::Verified,
        "{name}"
    );
    let report = check_exact(
        &serde_json::to_string(card).unwrap(),
        &printed,
        &library,
        &BTreeSet::from([mtg_cards::load::slug(name)]),
        &Caps::everything(),
    );
    assert_eq!(report.verdict(), Verdict::Accepted, "{name}: {:?}", report.clauses);
}
