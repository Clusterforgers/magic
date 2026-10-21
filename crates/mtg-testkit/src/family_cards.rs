//! Triggers on the creature an Equipment is attached to, on batches of events, on the nth event of a turn and on the amount
//! of an event, with conditions over the opponents' life, and the decks and games built from the corpus cards that use them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, burn, burn_own_creature, drain, free_creature, land, salve};
use crate::corpus::{self, CorpusError};

/// Lands in a families deck: black twice, blue, white, red and green in turn.
const LANDS: usize = 25;

/// The corpus cards of the trigger families, with their copies in a deck.
pub const FAMILY_CARDS: [(&str, usize); 8] = [
    ("Skullclamp", 3),
    ("Sword of Body and Mind", 2),
    ("The Thing, Ben Grimm", 2),
    ("Lat-Nam Adept", 3),
    ("Sanguine Bond", 3),
    ("Fear of Failed Tests", 3),
    ("Bloodghast", 3),
    ("Renewed Faith", 2),
];

/// The deck of a seat: lands in four colours, every family card, cards that draw several and cards that gain life, creatures
/// with one toughness for an Equipment to kill, and burn that kills one's own creature or hurts the opponents.
pub fn families_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Black,
        Color::Blue,
        Color::White,
        Color::Black,
        Color::Red,
        Color::Green,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 6])).collect();
    for (name, copies) in FAMILY_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(corpus::card("Brainstorm")?, 2));
    deck.extend(std::iter::repeat_n(free_creature("Mite", 1, 1), 4));
    deck.extend(std::iter::repeat_n(hero(), 4));
    deck.extend(std::iter::repeat_n(bear(), 4));
    deck.extend(std::iter::repeat_n(burn("Burn", 3), 3));
    deck.extend(std::iter::repeat_n(burn_own_creature("Backfire"), 2));
    deck.extend(std::iter::repeat_n(drain("Drain"), 3));
    deck.extend(std::iter::repeat_n(salve("Salve"), 3));
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A free 2/2 Hero, so a batch of damage from Heroes has more than one source.
fn hero() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Hero")
            .with_type(CardType::Creature)
            .with_subtype("Hero")
            .with_power_toughness(2, 2),
    )
}

/// A game of `players` seats with families decks.
pub fn families_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(families_deck(&mut GameRng::substream(seed, 21_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}
