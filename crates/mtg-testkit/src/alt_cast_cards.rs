//! The table of alternative costs (`Ability::AltCast`): one row per card, each a creature with the way it can be cast
//! for, shared by the scenario tests and the seeded fuzz decks.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::alt_riders::{alt_cast, impending_riders, warp_riders};
use mtg_cards::mana::ManaCostX;
use mtg_cards::value::CastWay;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};

/// One row of the table: a creature card with an alternative cost.
pub struct AltRow {
    /// The card name.
    pub name: &'static str,
    /// Its printed mana cost.
    pub cost: &'static str,
    /// The alternative cost: the way it is remembered by.
    pub way: CastWay,
    /// The mana of the alternative cost.
    pub mana: &'static str,
}

/// Every card of the table.
pub const ROWS: [AltRow; 4] = [
    AltRow {
        name: "Surge Beast",
        cost: "{4}{R}{R}",
        way: CastWay::Surge,
        mana: "{R}",
    },
    AltRow {
        name: "Web Beast",
        cost: "{3}{W}",
        way: CastWay::WebSlinging,
        mana: "{W}",
    },
    AltRow {
        name: "Warp Beast",
        cost: "{3}{R}",
        way: CastWay::Warp,
        mana: "{R}",
    },
    AltRow {
        name: "Impending Beast",
        cost: "{4}{G}",
        way: CastWay::Impending,
        mana: "{1}{G}",
    },
];

fn mana(text: &str) -> ManaCost {
    ManaCost::parse(text).unwrap_or_default()
}

/// The card of a row: a 3/3 creature with its alternative cost and what the way does to the permanent.
pub fn card(row: &AltRow) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(row.name)
        .with_type(CardType::Creature)
        .with_mana_cost(mana(row.cost))
        .with_power_toughness(3, 3)
        .with_rules(alt_cast(
            row.way,
            row.mana.parse::<ManaCostX>().unwrap_or_default(),
        ));
    let riders = match row.way {
        CastWay::Warp => warp_riders(),
        CastWay::Impending => impending_riders(4),
        _ => Vec::new(),
    };
    for rider in riders {
        card = card.with_rules(rider);
    }
    Arc::new(card)
}

/// The row of the card called `name`.
pub fn row(name: &str) -> Option<&'static AltRow> {
    ROWS.iter().find(|row| row.name == name)
}

/// A legal Normal deck of lands of the colours of the table, three of each card of the table and cheap creatures to cast
/// first (surge) and to attack with, derived entirely from `rng`.
pub fn alt_cast_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Red, Color::White, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 3])).collect();
    for row in &ROWS {
        deck.extend(std::iter::repeat_n(card(row), 3));
    }
    let mut slot = 0;
    while deck.len() < 60 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Alt Cast Fodder {slot} {power}/{power}");
        deck.extend(std::iter::repeat_n(
            creature(&name, 1, &[colors[rng.index(3)]], power, power),
            2,
        ));
        slot += 1;
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`alt_cast_deck`]s, fully determined by `seed`.
pub fn alt_cast_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| alt_cast_deck(&mut GameRng::substream(seed, 37_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
