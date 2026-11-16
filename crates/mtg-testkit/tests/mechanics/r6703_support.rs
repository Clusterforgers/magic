//! Goal: every card of the r67-03 pack plays whole: the corpus computes it `Verified`, with no clause left by hand.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::support::Support;
use mtg_testkit::corpus::corpus;
use rstest::rstest;

#[rstest]
#[case("Avatar Aang // Aang, Master of Elements")]
#[case("Jace, Multiverse Architect")]
#[case("Leonin Arbiter")]
#[case("Desertion")]
#[case("The Capitoline Triad")]
#[case("Brass Infiniscope")]
#[case("Ratchet, Field Medic // Ratchet, Rescue Racer")]
#[case("Chance for Glory")]
#[case("Sanctum Lurker")]
#[case("Taii Wakeen, Perfect Shot")]
#[case("Optimus Prime, Hero // Optimus Prime, Autobot Leader")]
fn the_card_plays_whole(#[case] name: &str) {
    let record = corpus().unwrap().support(name).unwrap();
    assert_eq!(
        record.support,
        Support::Verified,
        "{name}: {:?}",
        record.clauses
    );
}
