//! Goal: the color identity a definition derives equals Scryfall's `color_identity` for every card of the corpus (cost, indicator, rules text, basic land types and the second face of a double-faced card), so a deck checked by the engine is checked as the deck check of the server checks it, and the corpus offers commanders and singleton cards for the Commander decks of the fuzz games.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::support::Support;
use mtg_import::scryfall::cards;
use mtg_testkit::corpus::corpus;

const SCRYFALL: &str = include_str!("../../../mtg-cards/tests/fixtures/corpus_scryfall.json");

#[test]
fn the_derived_identity_is_scryfalls_for_every_corpus_card() {
    let corpus = corpus().unwrap();
    let mut wrong = Vec::new();
    let mut total = 0;
    for record in cards(SCRYFALL.as_bytes()) {
        let record = record.unwrap();
        let card = corpus.card(&record.name).unwrap();
        total += 1;
        if card.color_identity() != record.color_identity {
            wrong.push(format!(
                "{}: derived {:?}, Scryfall {:?}",
                record.name,
                card.color_identity(),
                record.color_identity
            ));
        }
    }
    assert!(total > 200, "the corpus is read: {total} cards");
    assert!(
        wrong.is_empty(),
        "{} of {total} differ:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

#[test]
fn the_corpus_has_commanders_and_cards_for_decks() {
    let corpus = corpus().unwrap();
    let mut leaders = 0;
    let mut singletons = 0;
    for name in corpus.names() {
        if corpus.support(name).unwrap().support != Support::Verified {
            continue;
        }
        let card = corpus.card(name).unwrap();
        leaders += usize::from(card.can_be_commander());
        singletons += usize::from(card.singleton_limit() == Some(1));
    }
    assert!(leaders >= 3, "{leaders} commanders");
    assert!(singletons >= 100, "{singletons} singleton cards");
}
