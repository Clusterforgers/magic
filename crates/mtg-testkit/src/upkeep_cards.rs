//! Echo, cumulative upkeep, undying and persist: synthetic creatures for the tables, and the decks and games built from
//! them beside the corpus cards that have the keywords.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, free_creature, keyworded, land};
use crate::corpus::{self, CorpusError};

/// Lands in a deck of these keywords, one color after the other.
const LANDS: usize = 20;

/// The corpus cards of an upkeep-keyword deck, with their copies.
pub const UPKEEP_CARDS: &[(&str, usize)] = &[
    ("Pouncing Jaguar", 2),
    ("Goblin War Buggy", 2),
    ("Rakdos Headliner", 2),
    ("Skizzik Surger", 1),
    ("Uktabi Efreet", 2),
    ("Morinfen", 1),
    ("Phyrexian Soulgorger", 2),
    ("Firestorm Hellkite", 1),
    ("Young Wolf", 3),
    ("Strangleroot Geist", 2),
    ("Vengeful Vampire", 1),
    ("Hound of Griselbrand", 1),
    ("Putrid Goblin", 3),
    ("Safehold Elite", 2),
    ("Scuzzback Marauders", 1),
    ("Kitchen Finks", 2),
    ("Lightning Bolt", 3),
    ("Doom Blade", 2),
];

/// A cost of mana such as `{1}`.
pub fn mana(text: &str) -> Cost {
    Cost::mana(text.parse().unwrap_or_default())
}

/// A cost of discarding a card.
pub fn discard_a_card() -> Cost {
    Cost(vec![CostItem::Discard {
        count: Amount::one(),
        filter: None,
    }])
}

/// A cost of sacrificing a creature.
pub fn sacrifice_a_creature() -> Cost {
    Cost(vec![CostItem::Sacrifice {
        filter: Selector(vec![Pred::Type(CardType::Creature)]),
        count: Amount::one(),
    }])
}

/// A cost of paying life.
pub fn pay_life(life: i32) -> Cost {
    Cost(vec![CostItem::PayLife(Amount::Fixed(life))])
}

/// A 2/2 that costs nothing and has echo with this cost.
pub fn echoer(name: &str, cost: Cost) -> Arc<CardDefinition> {
    keyworded(name, 2, 2, &[Keyword::Echo(cost)])
}

/// A 2/2 that costs nothing and has cumulative upkeep with this cost.
pub fn ager(name: &str, cost: Cost) -> Arc<CardDefinition> {
    keyworded(name, 2, 2, &[Keyword::CumulativeUpkeep(cost)])
}

/// A 2/2 that costs nothing and has undying.
pub fn undier(name: &str) -> Arc<CardDefinition> {
    keyworded(name, 2, 2, &[Keyword::Undying])
}

/// A 2/2 that costs nothing and has persist.
pub fn persister(name: &str) -> Arc<CardDefinition> {
    keyworded(name, 2, 2, &[Keyword::Persist])
}

/// A 2/2 that costs nothing and has both undying and persist.
pub fn both(name: &str) -> Arc<CardDefinition> {
    keyworded(name, 2, 2, &[Keyword::Undying, Keyword::Persist])
}

/// A legal Normal deck of sixty cards of the upkeep keywords, burn and removal that kill them, and small creatures to
/// sacrifice, derived entirely from `rng`.
pub fn upkeep_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Red,
        Color::Green,
        Color::Black,
        Color::White,
        Color::Blue,
    ];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in UPKEEP_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(both("Twice Dead"), 2));
    deck.extend(std::iter::repeat_n(free_creature("Mite", 1, 1), 3));
    deck.extend(std::iter::repeat_n(burn("Burn", 2), 2));
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`upkeep_deck`]s, fully determined by `seed`.
pub fn upkeep_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(upkeep_deck(&mut GameRng::substream(seed, 26_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: an upkeep-keyword deck is a legal sixty cards with every card of the list, fixed by its seed, and the synthetic creatures carry the keyword they are named for.
    use mtg_cards::ability::Ability;
    use mtg_cards::support::Support;

    use super::*;

    fn names(deck: &[Arc<CardDefinition>]) -> Vec<String> {
        deck.iter().map(|card| card.name.clone()).collect()
    }

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_list_and_is_fixed_by_its_seed() {
        let first = upkeep_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = upkeep_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(first.len(), 60);
        assert_eq!(names(&first), names(&again));
        assert_ne!(
            names(&first),
            names(&upkeep_deck(&mut GameRng::substream(2, 0)).unwrap())
        );
        for &(name, copies) in UPKEEP_CARDS {
            let held = first.iter().filter(|card| card.name == name).count();
            assert_eq!(held, copies, "{name}");
        }
    }

    #[test]
    fn the_corpus_cards_of_the_deck_are_verified() {
        let corpus = corpus::corpus().unwrap();
        for &(name, _) in UPKEEP_CARDS {
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}"
            );
        }
    }

    #[test]
    fn the_synthetic_creatures_carry_their_keyword() {
        let keywords = |card: &CardDefinition| -> Vec<Keyword> {
            card.rules
                .iter()
                .filter_map(|rules| match &**rules {
                    Ability::Keyword(keyword) => Some(keyword.clone()),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(
            keywords(&echoer("E", mana("{1}"))),
            [Keyword::Echo(mana("{1}"))]
        );
        assert_eq!(
            keywords(&ager("A", pay_life(1))),
            [Keyword::CumulativeUpkeep(pay_life(1))]
        );
        assert_eq!(keywords(&undier("U")), [Keyword::Undying]);
        assert_eq!(keywords(&persister("P")), [Keyword::Persist]);
        assert_eq!(keywords(&both("B")), [Keyword::Undying, Keyword::Persist]);
    }
}
