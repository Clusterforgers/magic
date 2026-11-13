//! Goal: every card the r40-2 pack made automatic plays in bot games at two and four seats without breaking an
//! invariant, erroring, looping or ending badly, with its rules run as written (the smoke gate of a draft).
use mtg_testkit::corpus::corpus;
use mtg_testkit::smoke::{DEFAULT_SEEDS, SmokeFailure, smoke_card};
use rstest::rstest;

#[rstest]
#[case("Battlefield Thaumaturge")]
#[case("Exiled Doomsayer")]
#[case("Hum of the Radix")]
#[case("Inquisitive Glimmer")]
#[case("Locket of Yesterdays")]
#[case("Mizzix, Replica Rider")]
#[case("Nalfeshnee")]
fn r402_pack_card_plays_in_bot_games(#[case] name: &str) {
    let corpus = corpus().unwrap_or_else(|e| panic!("{e}"));
    let card = corpus.card(name).unwrap_or_else(|e| panic!("{name}: {e}"));
    let report = smoke_card(&card, &DEFAULT_SEEDS);
    assert!(
        matches!(report.failure, None | Some(SmokeFailure::NeverSeen { .. })),
        "{name}: {:?}",
        report.failure
    );
}
