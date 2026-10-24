//! The blockers that alone held back two cards each in the third round: the decks and games built from the corpus cards that
//! a creature dealt damage by this creature, another creature that dies and returns, the free cast from the hand, bites,
//! the controller of the entering creature, the dealer of damage, shroud after an attach, a return from the graveyard
//! and the lands that reveal a card freed.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, free_creature, gain_life, land, salve};
use crate::corpus::{self, CorpusError};

/// Basic lands in a deck of these cards, one color after the other.
const LANDS: usize = 15;

/// The corpus cards of these blockers, with their copies in a deck.
pub const THIRD_CARDS: &[(&str, usize)] = &[
    ("Rienne, Angel of Rebirth", 1),
    ("Predator Ooze", 1),
    ("Vampiric Dragon", 1),
    ("Baral's Expertise", 1),
    ("Rishkar's Expertise", 1),
    ("Blood Seeker", 1),
    ("Suture Priest", 1),
    ("Silver Shroud Costume", 1),
    ("Silversmote Ghoul", 1),
    ("Super Combo", 1),
    ("It's Clobberin' Time!", 1),
    ("Rakish Heir", 1),
    ("Stensia Masquerade", 1),
    ("Flamekin Village", 1),
    ("Primal Beyond", 1),
    ("Gilt-Leaf Palace", 1),
    ("Murmuring Bosk", 1),
    ("Ancient Amphitheater", 1),
    ("Secluded Glen", 1),
    ("Wanderwine Hub", 1),
];

/// A creature card of this creature type, which the lands of these cards ask to reveal and Rakish Heir counts.
fn kin(name: &str, kind: &str, size: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(kind)
            .with_power_toughness(size, size),
    )
}

/// A legal Normal deck of sixty cards: these cards, life gain and burn for them to answer, creatures of the types the
/// lands reveal, and creatures, derived entirely from `rng`.
pub fn third_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in THIRD_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(salve("Third Salve"), 2));
    deck.extend(std::iter::repeat_n(gain_spell(), 2));
    deck.extend(std::iter::repeat_n(burn("Third Burn", 4), 3));
    let fodder = [
        kin("Spark", "Elemental", 1),
        kin("Flame", "Elemental", 2),
        kin("Bat", "Vampire", 2),
        kin("Fang", "Vampire", 3),
        kin("Sprite", "Faerie", 1),
        kin("Archer", "Elf", 2),
        free_creature("Brute", 3, 3),
        free_creature("Newt", 1, 1),
        free_creature("Boar", 3, 3),
        free_creature("Wolf", 2, 2),
    ];
    for card in fodder.iter().cycle().take(60 - deck.len()) {
        deck.push(card.clone());
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// An instant that gains three life, which the ghoul counts.
fn gain_spell() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Third Brew")
            .with_type(CardType::Instant)
            .with_mana_cost(crate::cards::cost(0, &[Color::Green]))
            .with_rules(mtg_cards::ability::Ability::Spell {
                targets: Vec::new(),
                effects: vec![gain_life(3)],
            }),
    )
}

/// A normal game of `players` seats whose decks are [`third_deck`]s, fully determined by `seed`.
pub fn third_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(third_deck(&mut GameRng::substream(seed, 34_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a third-round deck is a legal sixty cards with every card of the list, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_list_is_verified() {
        let deck = third_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(deck.len(), 60);
        let corpus = corpus::corpus().unwrap();
        for &(name, copies) in THIRD_CARDS {
            assert_eq!(deck.iter().filter(|card| card.name == name).count(), copies);
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}"
            );
        }
    }
}
