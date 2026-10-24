//! The chosen-player and party cards: creatures of the four party roles and the deck and games built from the corpus cards
//! that choose a player as they enter or count a party.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, land};
use crate::corpus::{self, CorpusError};

/// Lands in a deck of these cards, one color after the other.
const LANDS: usize = 24;

/// The roles of a party.
pub const ROLES: [&str; 4] = ["Cleric", "Rogue", "Warrior", "Wizard"];

/// The corpus cards that choose a player or count a party, with their copies in a deck.
pub const PARTY_CARDS: &[(&str, usize)] = &[
    ("True-Name Nemesis", 4),
    ("Sewer Nemesis", 4),
    ("Stuffy Doll", 4),
    ("Spoils of Adventure", 4),
    ("Malakir Blood-Priest", 4),
];

/// A creature with a cost, these creature types and these keywords.
pub fn typed_creature(
    name: &str,
    generic: u32,
    colored: &[Color],
    subtypes: &[&str],
    keywords: &[Keyword],
) -> Arc<CardDefinition> {
    let card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_mana_cost(cost(generic, colored))
        .with_power_toughness(1, 1);
    let card = subtypes
        .iter()
        .fold(card, |card, subtype| card.with_subtype(*subtype));
    Arc::new(keywords.iter().fold(card, |card, keyword| {
        card.with_rules(Ability::Keyword(keyword.clone()))
    }))
}

/// A creature that is only the role `role` names (an index into [`ROLES`]).
pub fn role_creature(name: &str, role: usize) -> Arc<CardDefinition> {
    typed_creature(name, 1, &[], &ROLES[role..=role], &[])
}

/// A legal Normal deck of sixty cards of the list, creatures of each role, a changeling and a creature of two roles,
/// derived entirely from `rng`.
pub fn party_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Blue, Color::Black, Color::White];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in PARTY_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (role, name) in ROLES.iter().enumerate() {
        deck.extend(std::iter::repeat_n(
            role_creature(&format!("Party {name}"), role),
            2,
        ));
    }
    deck.extend(std::iter::repeat_n(
        typed_creature("Party Shapeshifter", 1, &[], &[], &[Keyword::Changeling]),
        2,
    ));
    deck.extend(std::iter::repeat_n(
        typed_creature("Party Rogue Warrior", 1, &[], &["Rogue", "Warrior"], &[]),
        2,
    ));
    deck.extend(std::iter::repeat_n(
        typed_creature("Party Bystander", 1, &[], &["Elf"], &[]),
        60 - deck.len(),
    ));
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`party_deck`]s, fully determined by `seed`.
pub fn party_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(party_deck(&mut GameRng::substream(seed, 910_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a party deck is a legal sixty cards with every card of the list, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    fn names(deck: &[Arc<CardDefinition>]) -> Vec<String> {
        deck.iter().map(|card| card.name.clone()).collect()
    }

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_list_and_is_fixed_by_its_seed() {
        let first = party_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = party_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(first.len(), 60);
        assert_eq!(names(&first), names(&again));
        for &(name, copies) in PARTY_CARDS {
            assert_eq!(
                first.iter().filter(|card| card.name == name).count(),
                copies
            );
        }
    }

    #[test]
    fn the_corpus_cards_of_the_deck_are_verified() {
        let corpus = corpus::corpus().unwrap();
        for &(name, _) in PARTY_CARDS {
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}: {:?}",
                corpus.support(name).unwrap()
            );
        }
    }
}
