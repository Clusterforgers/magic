//! Goal: every boss precon deck file of `content/decks` is 100 cards, each card is in the test corpus and plays
//! automatically (Verified or Read), and each lies within the colour identity of the precon's face commander.

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

fn deck_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("content/decks")
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
fn every_boss_deck_card_plays_automatically_from_the_corpus() {
    let corpus = corpus::corpus().unwrap();
    let mut failures = Vec::new();
    for deck in decks() {
        for (name, _) in &deck.cards {
            match corpus.support(name) {
                Ok(record) if matches!(record.support, Support::Verified | Support::Read) => {}
                Ok(record) => failures.push(format!("{}: {name} is {:?}", deck.name, record.support)),
                Err(error) => failures.push(format!("{}: {error}", deck.name)),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_boss_deck_card_lies_within_its_commanders_colours() {
    for deck in decks() {
        let identity = deck
            .commander
            .iter()
            .flat_map(|face| corpus::card(face).unwrap().color_identity().iter().collect::<Vec<_>>())
            .collect::<Vec<_>>();
        for (name, _) in &deck.cards {
            let card = corpus::card(name).unwrap();
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
