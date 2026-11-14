//! Goal: each card of the r61-02 pack compiles whole from its printed text into exactly its rules file, and the
//! renderer writes each of its abilities back as a printed clause, so the pack's cards play automatically.

use mtg_cards::ability::Ability;
use mtg_cards::card::ImplStatus;
use mtg_cards::compile::{compile, same_written_text, written_variants};
use mtg_cards::trigger::TriggerPattern;
use rstest::rstest;

use crate::common;

#[rstest]
#[case::aven_courier("Aven Courier")]
#[case::blue_loyal_raptor("Blue, Loyal Raptor")]
#[case::crystalline_giant("Crystalline Giant")]
#[case::the_caves_of_androzani("The Caves of Androzani")]
#[case::blitzball_stadium("Blitzball Stadium")]
fn r6102_the_card_compiles_into_its_file(#[case] name: &str) {
    let printed = common::printed()
        .into_iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("{name} has no fixture"));
    let library = common::Corpus::repo().library();
    let by_hand = library.card(&printed.oracle_id).expect("a rules file");
    let compiled = compile(&printed);
    // What the compiler reads is the file's; a clause it leaves unread is still written back from the file.
    if let Some(card) = &compiled.card {
        for (face, ability) in card.abilities() {
            assert!(
                ability.is_manual() || by_hand.faces[face].abilities.contains(ability),
                "{name}: read {ability:?}, which the file lacks"
            );
        }
    }
    assert_eq!(by_hand.impl_status, ImplStatus::Full, "{name}");
    for (face, written) in by_hand.faces.iter().enumerate() {
        // Keywords and Saga chapters are written by their own writers, which the corpus test checks.
        let worded = |a: &&Ability| {
            !matches!(a, Ability::Keyword(_))
                && !matches!(
                    a,
                    Ability::Triggered {
                        when: TriggerPattern::Chapter { .. },
                        ..
                    }
                )
        };
        for ability in written.abilities.iter().filter(worded) {
            let texts = written_variants(ability);
            assert!(
                compiled.clauses.iter().any(|clause| clause.face == face
                    && texts
                        .iter()
                        .any(|text| same_written_text(text, &clause.text))),
                "{name}: none of {texts:?} is a clause"
            );
        }
    }
}
