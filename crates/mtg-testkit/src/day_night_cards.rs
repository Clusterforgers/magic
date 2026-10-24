//! Cards that make it day or night, and a seeded deck of them and the corpus cards that read the day and night, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::{Destination, Effect};
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::value::DayNight;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, land};
use crate::corpus;

/// The corpus cards of the day and night deck besides lands: permanents that start the day and act when it changes, a
/// spell that costs less at night, a permanent that changes the day by hand and cards that turn over with it.
pub const DAY_NIGHT_CARDS: &[&str] = &[
    "Sunrise Cavalier",
    "Firmament Sage",
    "Brimstone Vandal",
    "Gavony Dawnguard",
    "The Celestus",
    "Moonrager's Slash",
    "Tavern Ruffian // Tavern Smasher",
    "Bird Admirer // Wing Shredder",
];

/// A free instant: "It becomes day." or "It becomes night."
pub fn day_maker(name: &str, to: DayNight) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![Effect::BecomeDayNight { to }],
            }),
    )
}

/// A free sorcery: "Put all creature cards from your hand onto the battlefield."
pub fn mass_landing(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![Effect::Move {
                    what: Ref::Each(Selector(vec![
                        Pred::In(ZoneKind::Hand),
                        Pred::Type(CardType::Creature),
                        Pred::OwnedBy(Ref::You),
                    ])),
                    to: Destination::Battlefield {
                        tapped: false,
                        under: None,
                        transformed: false,
                        counters: Vec::new(),
                    },
                }],
            }),
    )
}

/// A legal Normal deck of lands, three of each card of [`DAY_NIGHT_CARDS`] and of the cards that make it day or night
/// and put creatures onto the battlefield together, derived entirely from `rng`.
pub fn day_night_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Red, Color::White, Color::Blue, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..28).map(|i| land(colors[i % 4])).collect();
    for card in DAY_NIGHT_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    deck.extend(std::iter::repeat_n(
        day_maker("Fuzz Dusk", DayNight::Night),
        3,
    ));
    deck.extend(std::iter::repeat_n(
        day_maker("Fuzz Dawn", DayNight::Day),
        3,
    ));
    deck.extend(std::iter::repeat_n(mass_landing("Fuzz Landing"), 2));
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`day_night_deck`]s, fully determined by `seed`.
pub fn day_night_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| day_night_deck(&mut GameRng::substream(seed, 71_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
