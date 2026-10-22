//! Goal: the oracle-text compiler turns every card of the design corpus into rules that load, match the imported text, earn the status their clauses say, and agree with the hand-written files on every keyword they read.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use mtg_cards::ability::Ability;
use mtg_cards::card::Card;
use mtg_cards::card::ImplStatus::Manual;
use mtg_cards::compile::{Compiled, compile};
use mtg_cards::load::canonical;
use mtg_cards::support::{PrintedCard, oracle_fingerprint};

use common::{Corpus, printed};

fn compiled_corpus() -> Vec<(PrintedCard, Compiled)> {
    printed()
        .into_iter()
        .map(|card| {
            let compiled = compile(&card);
            (card, compiled)
        })
        .collect()
}

#[test]
fn every_corpus_card_earns_the_status_its_clauses_say() {
    let problems = Corpus::repo().status_problems();
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn every_compiled_card_loads_and_matches_the_imported_text() {
    let problems = Corpus::repo().compiled_problems();
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn the_files_the_compiler_writes_are_canonical_and_read_back_unchanged() {
    for (_, compiled) in compiled_corpus() {
        let Some(card) = compiled.card else { continue };
        let text = canonical(&card);
        let again: Card = mtg_cards::load::parse_json("compiled", &text).unwrap();
        assert_eq!(again, card);
        assert_eq!(canonical(&again), text);
    }
}

#[test]
fn live_cards_carry_the_fingerprint_of_every_face() {
    for (printed, compiled) in compiled_corpus() {
        let Some(card) = compiled.card else { continue };
        for (face, shown) in card.faces.iter().zip(&printed.faces) {
            let expected = if card.impl_status == Manual {
                String::new()
            } else {
                oracle_fingerprint(&shown.oracle_text)
            };
            assert_eq!(face.oracle_fingerprint, expected, "{}", printed.name);
        }
    }
}

#[test]
fn clauses_lie_inside_the_text_and_never_overlap() {
    for (printed, compiled) in compiled_corpus() {
        for (at, clause) in compiled.clauses.iter().enumerate() {
            let text = &printed.faces[clause.face].oracle_text;
            let length = u32::try_from(text.chars().count()).unwrap();
            assert!(clause.span.start < clause.span.end && clause.span.end <= length);
            assert!(clause.span.slice(text).is_some(), "{}", printed.name);
            assert!(
                compiled.clauses[..at]
                    .iter()
                    .filter(|earlier| earlier.face == clause.face)
                    .all(|earlier| !earlier.span.overlaps(&clause.span)),
                "{}",
                printed.name
            );
        }
    }
}

#[test]
fn a_live_card_has_one_manual_ability_for_each_unread_clause() {
    for (printed, compiled) in compiled_corpus() {
        let Some(card) = compiled.card else { continue };
        let unread = compiled
            .clauses
            .iter()
            .filter(|clause| !clause.read)
            .count();
        let manual = card
            .abilities()
            .filter(|(_, ability)| ability.is_manual())
            .count();
        let expected = if card.impl_status == Manual {
            0
        } else {
            unread
        };
        assert_eq!(manual, expected, "{}", printed.name);
    }
}

/// The keywords the compiler reads are among the abilities of the hand-written file of the same card, face by face.
#[test]
fn the_keywords_read_are_in_the_hand_written_files() {
    let hand_written = Corpus::repo().library();
    for (printed, compiled) in compiled_corpus() {
        let Some(card) = compiled.card else { continue };
        let by_hand = hand_written.card(&printed.oracle_id).unwrap();
        for (face, ability) in card.abilities() {
            if matches!(ability, Ability::Keyword(_)) {
                assert!(
                    by_hand.faces[face].abilities.contains(ability),
                    "{}: {ability:?} is not in the hand-written file",
                    printed.name
                );
            }
        }
    }
}
