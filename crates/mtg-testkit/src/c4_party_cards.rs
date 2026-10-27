//! The party, chosen-player and prepare cards of (c14a) as a seeded fuzz deck of the corpus cards.

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
use crate::party_cards::{ROLES, role_creature, typed_creature};

/// The corpus cards of the deck with their copies.
pub const C4_PARTY_CARDS: &[(&str, usize)] = &[
    ("Squad Commander", 2),
    ("Burakos, Party Leader", 2),
    ("Nalia de'Arnise", 2),
    ("Tazri, Beacon of Unity", 2),
    ("Sawhorn Nemesis", 2),
    ("Saskia the Unyielding", 2),
    ("Dirgur Focusmage // Braingeyser", 2),
    ("Inspired Skypainter // Maestro's Gift", 2),
    ("Lorehold Archivist // Restore Relic", 2),
    ("Naktamun Lorespinner // Wheel of Fortune", 2),
    ("Striding Shotcaller // Run the Play", 2),
    ("Stensian Sanguinist // Exsanguinate", 2),
];

/// Lands in the deck, one color after the other.
const LANDS: usize = 22;

/// A sorcery for five mana that gains its caster a life: the big spell Dirgur Focusmage waits for.
fn big_spell() -> Arc<CardDefinition> {
    let gain: Ability =
        serde_json::from_str(r#"{"Spell":{"effects":[{"GainLife":{"amount":1}}]}}"#)
            .unwrap_or_else(|error| panic!("{error}"));
    Arc::new(
        CardDefinition::new("Big Spell")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(5, &[]))
            .with_rules(gain),
    )
}

/// A legal Normal deck of sixty cards: the lands, two of each card of the list, creatures of the four party roles, a
/// changeling and two big spells, derived entirely from `rng`.
pub fn c4_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in C4_PARTY_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (role, name) in ROLES.iter().enumerate() {
        deck.extend(std::iter::repeat_n(
            role_creature(&format!("Party {name}"), role),
            2,
        ));
    }
    deck.push(typed_creature(
        "Party Shapeshifter",
        1,
        &[],
        &[],
        &[Keyword::Changeling],
    ));
    deck.extend(std::iter::repeat_n(big_spell(), 2));
    deck.extend(std::iter::repeat_n(
        typed_creature("Party Bystander", 1, &[], &["Elf"], &[]),
        60 - deck.len(),
    ));
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`c4_deck`]s, fully determined by `seed`.
pub fn c4_party_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(c4_deck(&mut GameRng::substream(seed, 920_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: the deck is a legal sixty cards with every card of the list, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_list_and_is_fixed_by_its_seed() {
        let first = c4_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = c4_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(first.len(), 60);
        let names = |deck: &[Arc<CardDefinition>]| -> Vec<String> {
            deck.iter().map(|card| card.name.clone()).collect()
        };
        assert_eq!(names(&first), names(&again));
        for &(name, copies) in C4_PARTY_CARDS {
            let own = corpus::card(name).unwrap().name.clone();
            assert_eq!(
                first.iter().filter(|card| card.name == own).count(),
                copies,
                "{name}"
            );
        }
    }

    #[test]
    fn the_corpus_cards_of_the_deck_are_verified() {
        let corpus = corpus::corpus().unwrap();
        for &(name, _) in C4_PARTY_CARDS {
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}: {:?}",
                corpus.support(name).unwrap()
            );
        }
    }
}
