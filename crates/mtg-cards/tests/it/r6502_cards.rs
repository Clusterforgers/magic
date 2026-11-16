//! Goal: each card of the r65-02 pack is whole (`Full`), every ability its printed text compiles into is one of its rules
//! file, and the renderer writes each ability of the file back as a printed clause, so the pack's cards play
//! automatically and read back as their Oracle text.

use mtg_cards::ability::Ability;
use mtg_cards::card::ImplStatus;
use mtg_cards::compile::{compile, same_written_text, written_variants};
use rstest::rstest;

use crate::common;

#[rstest]
#[case::witness_protection("Witness Protection")]
#[case::the_curse_of_fenric("The Curse of Fenric")]
#[case::tenth_district_hero("Tenth District Hero")]
#[case::bronzehide_lion("Bronzehide Lion")]
fn r6502_the_card_compiles_into_its_file(#[case] name: &str) {
    let printed = common::printed()
        .into_iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("{name} has no fixture"));
    let library = common::Corpus::repo().library();
    let by_hand = library.card(&printed.oracle_id).expect("a rules file");
    let compiled = compile(&printed);
    let clauses: Vec<&str> = compiled.clauses.iter().map(|c| c.text.as_str()).collect();
    let card = compiled.card.as_ref().expect("a compiled card");
    for (face, ability) in card.abilities() {
        assert!(
            ability.is_manual() || by_hand.faces[face].abilities.contains(ability),
            "{name}: read {ability:?}, which the file lacks; clauses {clauses:?}"
        );
    }
    assert_eq!(by_hand.impl_status, ImplStatus::Full, "{name}");
    for (face, written) in by_hand.faces.iter().enumerate() {
        for ability in written
            .abilities
            .iter()
            .filter(|a| !matches!(a, Ability::Keyword(_)))
        {
            let texts = written_variants(ability);
            assert!(
                compiled.clauses.iter().any(|clause| clause.face == face
                    && texts
                        .iter()
                        .any(|text| same_written_text(text, &clause.text))),
                "{name}: none of {texts:?} is a clause of {clauses:?}"
            );
        }
    }
}
