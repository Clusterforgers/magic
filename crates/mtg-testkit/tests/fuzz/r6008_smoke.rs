//! Goal: every card the r60-08 pack made automatic plays in bot games at two and four seats without breaking an invariant,
//! erroring, looping or ending badly: the smoke gate passes it, or the games simply never met it.
use mtg_testkit::corpus::corpus;
use mtg_testkit::smoke::{DEFAULT_SEEDS, SmokeFailure, smoke_card};
use rstest::rstest;

#[rstest]
#[case("Agatha's Soul Cauldron")]
#[case("Dragon Cultist")]
#[case("Dungeon Delver")]
#[case("Grothama, All-Devouring")]
#[case("Niv-Mizzet Reborn")]
#[case("Pulmonic Sliver")]
#[case("Toralf, God of Fury // Toralf's Hammer")]
#[case("Virtus's Maneuver")]
fn r6008_pack_card_smokes(#[case] name: &str) {
    let corpus = corpus().unwrap_or_else(|e| panic!("{e}"));
    let card = corpus.card(name).unwrap_or_else(|e| panic!("{name}: {e}"));
    let report = smoke_card(&card, &DEFAULT_SEEDS);
    assert!(
        matches!(report.failure, None | Some(SmokeFailure::NeverSeen { .. })),
        "{name}: {:?}",
        report.failure
    );
}
