//! Umbra armor (CR 702.89) and the decks and games built from it: the four Umbra Auras of the precons with the destroy
//! effects, burn, regeneration and combat they are saved from.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, burn, elf, keyworded_creature, land};
use crate::corpus::{self, CorpusError};
use crate::prevent_cards::smite;

/// Lands in an umbra deck: five colours in turn.
const LANDS: usize = 22;

/// The cards of the corpus the umbra deck holds, with how many of each: the Auras, the destruction and the answers to it.
pub const UMBRA_CARDS: &[(&str, usize)] = &[
    ("Eel Umbra", 3),
    ("Snake Umbra", 3),
    ("Bear Umbra", 3),
    ("Octopus Umbra", 3),
    ("Death Ward", 2),
    ("Drudge Skeletons", 2),
    ("Wrath of God", 1),
    ("Doom Blade", 2),
    ("Lightning Bolt", 3),
    ("Fireball", 1),
    ("Swords to Plowshares", 1),
    ("Serra Angel", 2),
    ("Hero of Bladehold", 1),
    ("Bonecrusher Giant", 1),
];

/// A legal Normal deck of Umbra Auras and what they are saved from, derived entirely from `rng`: destroy effects (also
/// Wrath of God, whose destruction can't be regenerated), burn, creatures with deathtouch, regeneration to offer beside
/// the armor, and creatures to enchant.
pub fn umbra_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Blue,
        Color::Green,
        Color::White,
        Color::Black,
        Color::Red,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for &(name, copies) in UMBRA_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (card, copies) in [
        (smite("Smite")?, 2),
        (burn("Spark", 2), 2),
        (burn("Flame", 4), 1),
        (
            keyworded_creature("Viper", 0, &[Color::Black], 1, 1, &[Keyword::Deathtouch]),
            2,
        ),
        (bear(), 4),
        (elf(), 2),
    ] {
        deck.extend(std::iter::repeat_n(card, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`umbra_deck`]s, fully determined by `seed`.
pub fn umbra_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(umbra_deck(&mut GameRng::substream(seed, 24_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: an umbra deck is a legal sixty or more cards with every Umbra Aura, every destroy effect and every answer in it, the Auras carry the keyword, and the deck is fixed by its seed.
    use super::*;

    #[test]
    fn an_umbra_deck_holds_every_card_and_is_fixed_by_its_seed() {
        let deck = umbra_deck(&mut GameRng::substream(7, 1)).unwrap();
        assert!(deck.len() >= 60, "{}", deck.len());
        for &(name, _) in UMBRA_CARDS {
            assert!(deck.iter().any(|card| card.name == name), "{name}");
        }
        let again = umbra_deck(&mut GameRng::substream(7, 1)).unwrap();
        let names = |deck: &[Arc<CardDefinition>]| -> Vec<String> {
            deck.iter().map(|card| card.name.clone()).collect()
        };
        assert_eq!(names(&deck), names(&again));
    }

    #[test]
    fn the_umbra_auras_of_the_corpus_carry_the_keyword() {
        for name in ["Eel Umbra", "Snake Umbra", "Bear Umbra", "Octopus Umbra"] {
            let card = corpus::card(name).unwrap();
            let text = format!("{:?}", card.rules);
            assert!(text.contains("UmbraArmor"), "{name}: {text}");
        }
    }
}
