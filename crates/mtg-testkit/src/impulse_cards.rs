//! A seeded deck of cards that exile the top of the library and let their controller play what they exile (impulse
//! draw): Abbot of Keral Keep, Professional Face-Breaker, Reckless Impulse and spells of the tests, over lands of both
//! their colours and creatures and instants of every cost for them to find, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::impulse;
use mtg_cards::modify::Duration;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, creature, instant, land};
use crate::corpus;

/// The corpus cards the impulse deck is made of besides lands and the cards found: what exiles.
pub const IMPULSE_CARDS: [&str; 5] = [
    "Abbot of Keral Keep",
    "Professional Face-Breaker",
    "Reckless Impulse",
    "Lightning Bolt",
    "Monastery Swiftspear",
];

/// A spell of the given type (`Sorcery` or `Instant`) that exiles the top `count` cards of its controller's library and
/// lets them play those cards until `duration` ends.
pub fn impulse_spell(
    name: &str,
    kind: CardType,
    generic: u32,
    colored: &[Color],
    count: i32,
    duration: Duration,
) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_mana_cost(cost(generic, colored))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: impulse(Amount::Fixed(count), duration),
            }),
    )
}

/// A legal Normal deck of lands, three of each such card, instants that impulse draw and creatures and instants of value
/// 1 to 4, derived entirely from `rng`.
pub fn impulse_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Red, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..30).map(|i| land(colors[i % 2])).collect();
    for card in IMPULSE_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    let flash = impulse_spell(
        "Quick Impulse",
        CardType::Instant,
        0,
        &[Color::Red],
        1,
        Duration::UntilEndOfTurn,
    );
    let long = impulse_spell(
        "Long Impulse",
        CardType::Instant,
        1,
        &[Color::Green],
        2,
        Duration::UntilEndOfYourNextTurn,
    );
    deck.extend(std::iter::repeat_n(flash, 3));
    deck.extend(std::iter::repeat_n(long, 3));
    for value in 1u32..=4 {
        let name = format!("Impulse Fodder {value}");
        let power = i32::try_from(1 + value % 3).unwrap_or(1);
        deck.extend(std::iter::repeat_n(
            creature(&name, value, &[], power, 2),
            2,
        ));
        let name = format!("Impulse Trick {value}");
        deck.push(instant(&name, value, &[]));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`impulse_deck`]s, fully determined by `seed`.
pub fn impulse_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| impulse_deck(&mut GameRng::substream(seed, 50_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
