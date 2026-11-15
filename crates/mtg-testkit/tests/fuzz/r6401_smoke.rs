//! Goal: every card the r64-01 pack plays whole runs in bot games at two and four seats without breaking an invariant,
//! erroring, looping or ending badly: the smoke gate passes it, or the games simply never met it.
use mtg_testkit::corpus::corpus;
use mtg_testkit::smoke::{DEFAULT_SEEDS, SmokeFailure, smoke_card};
use rstest::rstest;

#[rstest]
#[case("Aegis of the Legion")]
#[case("Aminatou, the Fateshifter")]
#[case("Desolation")]
#[case("Drop Tower")]
#[case("Inniaz, the Gale Force")]
#[case("Order of Succession")]
#[case("Risky Move")]
#[case("Tunnel of Love")]
fn r6401_pack_card_plays_in_bot_games(#[case] name: &str) {
    let corpus = corpus().unwrap_or_else(|e| panic!("{e}"));
    let card = corpus.card(name).unwrap_or_else(|e| panic!("{name}: {e}"));
    let report = smoke_card(&card, &DEFAULT_SEEDS);
    assert!(
        matches!(report.failure, None | Some(SmokeFailure::NeverSeen { .. })),
        "{name}: {:?}",
        report.failure
    );
}
