//! Goal: every boss precon deck file of `content/decks` is 100 cards with its face commander shuffled in; each card
//! plays automatically from the test corpus (Verified or Read) or from a whole draft of the read tier, unless it waits
//! on the pending list; and each corpus card lies within the colour identity of the precon's face commander.

use std::path::{Path, PathBuf};

use mtg_cards::support::Support;
use mtg_testkit::corpus;
use serde::Deserialize;

/// A deck file as `mtg-cli export-precon` writes it.
#[derive(Debug, Deserialize)]
struct DeckFile {
    name: String,
    commander: Vec<String>,
    cards: Vec<(String, u32)>,
}

/// Deck cards that play neither from the corpus nor from a sound draft, each with what holds it back (roadmap dl1).
const PENDING: &[&str] = &[
    // The renderer cannot write its compiled exile-until as the two printed sentences, so it is not a corpus card.
    "Leonin Relic-Warder",
    // Read wrong: its activation cannot count its own targets for the reduction.
    "Wayta, Trainer Prodigy",
    // Read wrong: proliferate cannot bind the permanents it put counters on.
    "Ripples of Potential",
];

fn deck_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("content/decks")
}

/// The draft of the read tier for a card, if `mtg-cli` holds one.
fn draft(name: &str) -> Option<serde_json::Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../mtg-cli/drafts")
        .join(format!("{}.json", mtg_cards::load::slug(name)));
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn decks() -> Vec<DeckFile> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(deck_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path).unwrap();
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        })
        .collect()
}

/// Whether the corpus plays the card automatically.
fn automatic_in_corpus(name: &str) -> bool {
    corpus::corpus()
        .unwrap()
        .support(name)
        .is_ok_and(|record| matches!(record.support, Support::Verified | Support::Read))
}

#[test]
fn every_boss_deck_is_a_hundred_cards() {
    let decks = decks();
    assert!(decks.len() >= 19, "the nineteen boss precons");
    for deck in &decks {
        let size: u32 = deck.cards.iter().map(|(_, count)| count).sum();
        assert_eq!(size, 100, "{}", deck.name);
        assert!(
            deck.commander
                .iter()
                .all(|face| deck.cards.iter().any(|(name, _)| name == face)),
            "{}: the face commander is shuffled in",
            deck.name
        );
    }
}

#[test]
fn every_boss_deck_card_plays_automatically() {
    let mut failures = Vec::new();
    for deck in decks() {
        for (name, _) in &deck.cards {
            if PENDING.contains(&name.as_str()) || automatic_in_corpus(name) {
                continue;
            }
            let whole = draft(name).is_some_and(|rules| {
                rules["impl_status"] == "Full" && !rules.to_string().contains("\"Manual\"")
            });
            if !whole {
                failures.push(format!("{}: {name}", deck.name));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_pending_card_is_a_deck_card_that_still_waits() {
    let decks = decks();
    for name in PENDING {
        assert!(
            decks
                .iter()
                .any(|deck| deck.cards.iter().any(|(card, _)| card == name)),
            "{name} is in no boss deck"
        );
        assert!(
            !automatic_in_corpus(name),
            "{name} plays automatically now: drop it from PENDING"
        );
    }
}

#[test]
fn every_boss_deck_card_lies_within_its_commanders_colours() {
    let corpus = corpus::corpus().unwrap();
    for deck in decks() {
        let identity = deck
            .commander
            .iter()
            .filter_map(|face| corpus.card(face).ok())
            .flat_map(|card| card.color_identity().iter().collect::<Vec<_>>())
            .collect::<Vec<_>>();
        if identity.is_empty() {
            // A commander that plays from a draft is not a corpus card; its deck is checked by its precon.
            continue;
        }
        for (name, _) in &deck.cards {
            let Ok(card) = corpus.card(name) else {
                continue;
            };
            for color in card.color_identity().iter() {
                assert!(
                    identity.contains(&color),
                    "{}: {name} is outside the commander's colours",
                    deck.name
                );
            }
        }
    }
}
