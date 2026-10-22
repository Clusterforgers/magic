//! Vehicles for the fuzz games: the corpus Vehicles that crew (Smuggler's Copter, Skysovereign, Goliath Truck, Thopter
//! Fabricator) and Vehicles of every crew number, beside the creatures that pay for them and the lands that cast them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, creature, land};
use crate::corpus;

/// The corpus Vehicles the games draw.
pub const VEHICLE_CARDS: &[&str] = &[
    "Smuggler's Copter",
    "Skysovereign, Consul Flagship",
    "Goliath Truck",
    "Thopter Fabricator",
];

/// A Vehicle for `generic` mana with this power and toughness and Crew `crew`.
fn vehicle(
    name: &str,
    generic: u32,
    (power, toughness): (i32, i32),
    crew: u32,
) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(generic, &[]))
            .with_power_toughness(power, toughness)
            .with_rules(Ability::Keyword(Keyword::Crew(crew))),
    )
}

/// A legal Normal deck of lands, the corpus Vehicles, plain Vehicles with Crew 1 to 4 and creatures of every power,
/// derived entirely from `rng`.
pub fn vehicle_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Red, Color::White, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 3])).collect();
    let mut cards: Vec<Arc<CardDefinition>> = VEHICLE_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
        .collect();
    for crew in 1..=4 {
        let size = i32::try_from(crew + 1).unwrap_or(2);
        cards.push(vehicle(
            &format!("Plain Vehicle {crew}"),
            crew + 1,
            (size, size),
            crew,
        ));
    }
    for card in cards {
        deck.extend(std::iter::repeat_n(card, 2));
    }
    for slot in 0..20 {
        let power = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let name = format!("Crew Member {slot} {power}/{power}");
        deck.push(creature(&name, 1, &[colors[rng.index(3)]], power, power));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`vehicle_deck`]s, fully determined by `seed`.
pub fn vehicle_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| vehicle_deck(&mut GameRng::substream(seed, 23_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
