//! Goal: every mutation kind finds its node in some corpus card and yields one fault there: a mutant still loads as
//! rules, differs from the original, changes the node it names, and is the same for the same seed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::builtin;
use mtg_cards::card::Card;
use mtg_testkit::mutate::{MutationKind, mutant, mutants};
use rstest::rstest;

/// The corpus cards, sorted by name.
fn corpus() -> Vec<&'static Card> {
    let mut cards: Vec<&Card> = builtin::library().unwrap().cards().collect();
    cards.sort_by(|a, b| a.name.cmp(&b.name));
    cards
}

#[rstest]
#[case::number(MutationKind::Number)]
#[case::condition(MutationKind::Condition)]
#[case::may(MutationKind::May)]
#[case::another(MutationKind::Another)]
#[case::filter(MutationKind::Filter)]
#[case::player(MutationKind::Player)]
#[case::duration(MutationKind::Duration)]
#[case::clause(MutationKind::Clause)]
fn each_kind_mutates_a_corpus_card_at_the_node_it_names(#[case] kind: MutationKind) {
    let cards = corpus();
    let (card, found) = cards
        .iter()
        .find_map(|card| mutant(card, kind, 0).map(|m| (*card, m)))
        .unwrap_or_else(|| panic!("no corpus card has a {} node", kind.name()));
    assert_eq!(found.kind, kind);
    assert_ne!(
        &found.card, card,
        "{}: the mutant equals the original",
        card.name
    );
    let before = serde_json::to_value(card).unwrap();
    let after = serde_json::to_value(&found.card).unwrap();
    assert_ne!(
        before.pointer(&found.at),
        after.pointer(&found.at),
        "{}: {} unchanged",
        card.name,
        found.at
    );
    assert_eq!(
        mutant(card, kind, 0),
        Some(found),
        "the same seed gives the same mutant"
    );
}

#[test]
fn a_card_without_the_node_has_no_mutant_of_that_kind() {
    let cards = corpus();
    let vanilla = cards
        .iter()
        .find(|card| card.faces.iter().all(|face| face.abilities.is_empty()))
        .unwrap();
    assert!(
        mutants(vanilla, 0).is_empty(),
        "{} has mutants",
        vanilla.name
    );
}

#[test]
fn the_seed_moves_the_fault_among_the_candidates() {
    let cards = corpus();
    let moved = cards.iter().any(|card| {
        let first = mutant(card, MutationKind::Number, 0).map(|m| m.at);
        (1..4).any(|seed| mutant(card, MutationKind::Number, seed).map(|m| m.at) != first)
    });
    assert!(moved, "no card's number fault moves with the seed");
}
