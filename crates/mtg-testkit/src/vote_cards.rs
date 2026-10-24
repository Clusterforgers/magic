//! Votes: the secret ballot for the tests of what each seat sees, and the deck and games built from the corpus cards
//! that vote.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::{Effect, VoteFor};
use mtg_cards::refs::Ref;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, free_creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a deck of these cards, one color after the other.
const LANDS: usize = 24;

/// The corpus cards that vote, with their copies in a deck.
pub const VOTE_CARDS: &[(&str, usize)] = &[
    ("Council's Judgment", 3),
    ("Magister of Worth", 3),
    ("Custodi Squire", 3),
];

/// A sorcery for `{U}`: "Each player secretly votes for heads or tails, then those votes are revealed. You draw a card
/// for each heads vote."
pub fn secret_ballot(name: &str) -> Arc<CardDefinition> {
    ballot(name, true)
}

/// The same sorcery with the votes cast in the open: "Each player votes for heads or tails."
pub fn open_ballot(name: &str) -> Arc<CardDefinition> {
    ballot(name, false)
}

/// A sorcery for `{U}` that holds a vote on heads or tails, secret or open, and draws a card for each heads vote.
fn ballot(name: &str, secret: bool) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[Color::Blue]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![
                    Effect::Vote {
                        options: VoteFor::Named(vec!["heads".to_owned(), "tails".to_owned()]),
                        secret,
                        bind: None,
                    },
                    Effect::Draw {
                        who: Ref::You,
                        count: Amount::Players(Ref::Bound("heads".to_owned())),
                    },
                ],
            }),
    )
}

/// A legal Normal deck of sixty cards of the list and plain creatures, derived entirely from `rng`.
pub fn vote_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::White, Color::Black, Color::Blue, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in VOTE_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(secret_ballot("Ballot"), 3));
    let mut slot = 0;
    while deck.len() < 60 {
        let power = 1 + slot % 4;
        let card = free_creature(&format!("Vote Fodder {slot}"), power, power);
        deck.extend(std::iter::repeat_n(card, 2.min(60 - deck.len())));
        slot += 1;
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`vote_deck`]s, fully determined by `seed`.
pub fn vote_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(vote_deck(&mut GameRng::substream(seed, 44_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a vote deck is a legal sixty cards with every card of the list, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    fn names(deck: &[Arc<CardDefinition>]) -> Vec<String> {
        deck.iter().map(|card| card.name.clone()).collect()
    }

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_list_and_is_fixed_by_its_seed() {
        let first = vote_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = vote_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(first.len(), 60);
        assert_eq!(names(&first), names(&again));
        for &(name, copies) in VOTE_CARDS {
            assert_eq!(
                first.iter().filter(|card| card.name == name).count(),
                copies
            );
        }
    }

    #[test]
    fn the_cards_of_the_list_are_verified() {
        for &(name, _) in VOTE_CARDS {
            let record = corpus::corpus().unwrap().support(name).unwrap();
            assert_eq!(record.support, Support::Verified, "{name}");
        }
    }
}
