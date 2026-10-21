//! Protection (CR 702.16) and the decks and games built from it: the corpus cards that have it or give it, with the
//! spells, Auras and Equipment it answers.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, burn, creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a protection deck: five colours in turn.
const LANDS: usize = 22;

/// Cards of the corpus that have protection or give it, with how many of each.
pub const PROTECT_CARDS: [(&str, usize); 3] = [
    ("Black Knight", 3),
    ("Kor Firewalker", 3),
    ("Mother of Runes", 3),
];

/// Cards of the corpus that damage, enchant, equip, target and block them.
const CORPUS_CARDS: [(&str, usize); 11] = [
    ("Lightning Bolt", 3),
    ("Burst Lightning", 1),
    ("Pacifism", 2),
    ("Bonesplitter", 2),
    ("Serra Angel", 2),
    ("Goblin Chieftain", 2),
    ("Swords to Plowshares", 1),
    ("Doom Blade", 1),
    ("Hero of Bladehold", 1),
    ("Giant Growth", 1),
    ("Wrath of God", 1),
];

/// A legal Normal deck of the protection cards and the cards they answer, derived entirely from `rng`.
pub fn protect_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::White,
        Color::Green,
        Color::Blue,
        Color::Red,
        Color::Black,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for (name, copies) in PROTECT_CARDS.into_iter().chain(CORPUS_CARDS) {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (card, copies) in [
        (burn("Spark", 2), 3),
        (bear(), 4),
        (creature("Ember Imp", 1, &[Color::Red], 2, 1), 3),
        (creature("Pale Knight", 1, &[Color::White], 2, 2), 3),
    ] {
        deck.extend(std::iter::repeat_n(card, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`protect_deck`]s, fully determined by `seed`.
pub fn protect_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(protect_deck(&mut GameRng::substream(seed, 23_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a protection deck is a legal sixty or more cards with every protection card and every card they answer in it, fixed by its seed, and the three protection cards of the corpus carry the keyword or the colour choice.
    use super::*;

    #[test]
    fn a_protection_deck_holds_every_card_and_is_fixed_by_its_seed() {
        let deck = protect_deck(&mut GameRng::substream(7, 1)).unwrap();
        assert!(deck.len() >= 60, "{}", deck.len());
        for (name, _) in PROTECT_CARDS.into_iter().chain(CORPUS_CARDS) {
            assert!(deck.iter().any(|card| card.name == name), "{name}");
        }
        let again = protect_deck(&mut GameRng::substream(7, 1)).unwrap();
        let names = |deck: &[Arc<CardDefinition>]| -> Vec<String> {
            deck.iter().map(|card| card.name.clone()).collect()
        };
        assert_eq!(names(&deck), names(&again));
    }

    #[test]
    fn the_protection_cards_of_the_corpus_carry_their_protection() {
        for (name, protection) in [
            ("Black Knight", "Protection"),
            ("Kor Firewalker", "Protection"),
            ("Mother of Runes", "AddProtectionFromChosenColor"),
        ] {
            let card = corpus::card(name).unwrap();
            let text = format!("{:?}", card.rules);
            assert!(text.contains(protection), "{name}: {text}");
        }
    }
}
