//! Goal: every card of the pack158-other pack plays automatically: its support with the engine's capabilities is
//! `Verified`, with no clause left to the table.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::support::Support;
use mtg_testkit::corpus::corpus;
use rstest::rstest;

#[rstest]
#[case::reprieve("Reprieve")]
#[case::firkraag("Firkraag, Cunning Instigator")]
#[case::urza("Urza, Planeswalker")]
fn p158_the_card_is_verified(#[case] name: &str) {
    let support = corpus().unwrap().support(name).unwrap();
    assert_eq!(support.support, Support::Verified, "{name}: {support:?}");
}
