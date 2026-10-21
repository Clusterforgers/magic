//! A seeded deck of cards that cast other cards during a resolution: Bloodbraid Elf (cascade) and a creature that
//! discovers when it enters, with lands of their colours, spells that need targets or ask modes, an X spell and
//! creatures of every cost for them to find, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::discover;
use mtg_cards::refs::{Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, creature, land};
use crate::corpus;

/// The corpus cards the cascade deck is made of besides lands and creatures: what is cast and what is found.
pub const CASCADE_CARDS: [&str; 5] = [
    "Bloodbraid Elf",
    "Lightning Bolt",
    "Izzet Charm",
    "Fireball",
    "Doom Blade",
];

/// A creature for `{1}{R}` that discovers 4 when it enters.
pub fn discoverer() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Discoverer")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(1, &[Color::Red]))
            .with_power_toughness(2, 1)
            .with_rules(Ability::Triggered {
                when: TriggerPattern::ZoneChange {
                    what: Selector::this(),
                    from: None,
                    to: Some(ZoneKind::Battlefield),
                },
                intervening_if: None,
                limit: None,
                from: ZoneKind::Battlefield,
                targets: Vec::new(),
                effects: discover(Amount::Fixed(4)),
            }),
    )
}

/// A legal Normal deck of lands, three of each such card, discoverers and creatures of value 1 to 5, derived entirely from
/// `rng`.
pub fn cascade_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Red, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..32).map(|i| land(colors[i % 2])).collect();
    for card in CASCADE_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    deck.extend(std::iter::repeat_n(discoverer(), 3));
    for value in 1u32..=5 {
        let name = format!("Cascade Fodder {value}");
        let power = i32::try_from(1 + value % 3).unwrap_or(1);
        let card = creature(&name, value, &[], power, 2);
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`cascade_deck`]s, fully determined by `seed`.
pub fn cascade_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| cascade_deck(&mut GameRng::substream(seed, 40_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
