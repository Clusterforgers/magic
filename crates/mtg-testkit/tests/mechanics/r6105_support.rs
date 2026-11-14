//! Goal: every card the r61-05 pack plays automatically is `Verified` with this engine's capabilities: no clause is left
//! to the table.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::support::Support;
use mtg_testkit::corpus::corpus;
use rstest::rstest;

#[rstest]
#[case::mind_bomb("Mind Bomb")]
#[case::library_of_leng("Library of Leng")]
#[case::primeval_spawn("Primeval Spawn")]
#[case::tariff("Tariff")]
#[case::braids("Braids, Arisen Nightmare")]
#[case::kodama("Kodama of the East Tree")]
#[case::dont_blink("Don't Blink")]
#[case::sarcophagus("Abandoned Sarcophagus")]
#[case::arachne("Arachne, Psionic Weaver")]
fn r6105_the_card_plays_whole(#[case] name: &str) {
    let record = corpus().unwrap().support(name).unwrap();
    assert_eq!(
        record.support,
        Support::Verified,
        "{name}: {:?}",
        record.clauses
    );
}
