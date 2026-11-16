//! Goal: every card of the r66-03 pack plays automatically: its support with the engine's capabilities is `Verified`,
//! with no clause left to the table.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::support::Support;
use mtg_testkit::corpus::corpus;
use rstest::rstest;

#[rstest]
#[case::sephiroth("Sephiroth, Fabled SOLDIER // Sephiroth, One-Winged Angel")]
#[case::hellkite_courser("Hellkite Courser")]
#[case::doubling_cube("Doubling Cube")]
#[case::the_master_multiplied("The Master, Multiplied")]
#[case::alms_collector("Alms Collector")]
#[case::comet_stellar_pup("Comet, Stellar Pup")]
fn r6603_the_card_is_verified(#[case] name: &str) {
    let support = corpus().unwrap().support(name).unwrap();
    assert_eq!(support.support, Support::Verified, "{name}: {support:?}");
}
