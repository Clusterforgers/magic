//! Synthetic creatures and spells for the keywords that are triggered abilities (ward, prowess, battle cry), and the
//! decks and games built from them beside the corpus cards that have them: Monastery Swiftspear, Hulking Raptor and
//! Hero of Bladehold.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{keyworded_creature, land, pinger};
use crate::corpus::{self, CorpusError};
use crate::pump_cards::grant;

/// Lands in a deck of these keywords.
const LANDS: usize = 24;

/// Ward with a cost of mana, such as `{1}` or `{2}`.
pub fn ward(cost: &str) -> Keyword {
    Keyword::Ward(Cost::mana(cost.parse().unwrap_or_default()))
}

/// A creature for `{1}{R}` with battle cry.
pub fn warlord() -> Arc<CardDefinition> {
    keyworded_creature(
        "Keyword Warlord",
        1,
        &[Color::Red],
        2,
        2,
        &[Keyword::BattleCry],
    )
}

/// A creature for `{1}{G}` with ward `{1}`.
pub fn sentinel() -> Arc<CardDefinition> {
    keyworded_creature("Keyword Sentinel", 1, &[Color::Green], 2, 2, &[ward("{1}")])
}

/// A creature for `{2}{W}` with ward `{2}` and prowess.
pub fn elk() -> Arc<CardDefinition> {
    keyworded_creature(
        "Keyword Elk",
        2,
        &[Color::White],
        3,
        2,
        &[ward("{2}"), Keyword::Prowess],
    )
}

/// A legal Normal deck in red, green and white of sixty cards whose creatures have ward, prowess and battle cry and
/// whose spells aim at them, give them the keywords or pump them: Monastery Swiftspear, Hulking Raptor and Hero of
/// Bladehold of the corpus, three creatures of its own, three spells that give a keyword until end of turn, Giant
/// Growth, Lightning Bolt, Swords to Plowshares, a creature with an ability that targets, and lands.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn keyword_triggers_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Red, Color::Green, Color::White];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 3])).collect();
    for (name, copies) in [
        ("Monastery Swiftspear", 4),
        ("Hulking Raptor", 4),
        ("Hero of Bladehold", 3),
        ("Lightning Bolt", 4),
        ("Giant Growth", 3),
        ("Swords to Plowshares", 2),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (card, copies) in [
        (warlord(), 4),
        (sentinel(), 4),
        (elk(), 3),
        (grant("Keyword Rally", Keyword::BattleCry), 1),
        (grant("Keyword Spark", Keyword::Prowess), 1),
        (grant("Keyword Ward", ward("{1}")), 1),
        (pinger("Keyword Pinger", 1), 2),
    ] {
        deck.extend(std::iter::repeat_n(card, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`keyword_triggers_deck`]s, fully determined by `seed`.
pub fn keyword_triggers_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(keyword_triggers_deck(&mut GameRng::substream(
            seed,
            12_000 + seat,
        ))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: the deck of the keywords that are triggers is a legal sixty cards, every card of it that the engine's capabilities cover is Verified or plain, and the same seed builds the same deck.
    use mtg_cards::support::Support;

    use super::*;

    #[test]
    fn the_deck_has_sixty_cards() {
        let deck = keyword_triggers_deck(&mut GameRng::substream(1, 1)).unwrap();
        assert_eq!(deck.len(), 60);
    }

    #[test]
    fn the_same_seed_builds_the_same_deck() {
        let names = |seed: u64| -> Vec<String> {
            keyword_triggers_deck(&mut GameRng::substream(seed, 5))
                .unwrap()
                .iter()
                .map(|card| card.name.clone())
                .collect()
        };
        assert_eq!(names(3), names(3));
        assert_ne!(names(3), names(4));
    }

    #[test]
    fn the_corpus_cards_of_the_deck_are_verified() {
        let corpus = corpus::corpus().unwrap();
        for name in [
            "Monastery Swiftspear",
            "Hulking Raptor",
            "Hero of Bladehold",
        ] {
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}"
            );
        }
    }

    #[test]
    fn ward_names_a_cost_of_mana() {
        assert_eq!(
            ward("{1}"),
            Keyword::Ward(Cost::mana("{1}".parse().unwrap()))
        );
    }
}
