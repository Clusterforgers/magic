//! Cards that creatures and graveyard cards help pay for (convoke, delve), beside the corpus cards that have the
//! keywords, and a seeded deck of them for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::{ManaCost, ManaSymbol};
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, creature, creature_costing, keyworded_creature, land};
use crate::corpus::{self, CorpusError};

fn rules(json: &str) -> Result<Ability, CorpusError> {
    serde_json::from_str(json).map_err(|error| CorpusError::Data(format!("{json}: {error}")))
}

fn with_keywords(card: CardDefinition, keywords: &[Keyword]) -> CardDefinition {
    keywords.iter().fold(card, |card, keyword| {
        card.with_rules(Ability::Keyword(keyword.clone()))
    })
}

/// A 2/2 whose cost is made of exactly these symbols, with these keywords.
pub fn helped_creature(
    name: &str,
    symbols: &[ManaSymbol],
    keywords: &[Keyword],
) -> Arc<CardDefinition> {
    let base = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_mana_cost(ManaCost::from_symbols(symbols.iter().copied()))
        .with_power_toughness(2, 2);
    Arc::new(with_keywords(base, keywords))
}

/// A sorcery for `{X}{R}` with these keywords: it deals X damage to any target.
pub fn helped_blaze(name: &str, keywords: &[Keyword]) -> Result<Arc<CardDefinition>, CorpusError> {
    let cost = ManaCost::parse("{R}").map_err(|error| CorpusError::Data(error.to_string()))?;
    let base = CardDefinition::new(name)
        .with_type(CardType::Sorcery)
        .with_mana_cost(cost)
        .with_x_in_cost(1)
        .with_rules(rules(
            r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"X"}}]}}"#,
        )?);
    Ok(Arc::new(with_keywords(base, keywords)))
}

/// The cards of this module and the corpus cards with convoke or delve, by name.
pub fn helped_card(name: &str) -> Result<Arc<CardDefinition>, CorpusError> {
    let hybrid = [
        ManaSymbol::Generic(2),
        ManaSymbol::Hybrid(Color::White, Color::Blue),
        ManaSymbol::Hybrid(Color::White, Color::Blue),
    ];
    let phyrexian = [ManaSymbol::Generic(1), ManaSymbol::Phyrexian(Color::Red)];
    Ok(match name {
        "Convoke Hybrid" => helped_creature(name, &hybrid, &[Keyword::Convoke]),
        "Delve Hybrid" => helped_creature(name, &hybrid, &[Keyword::Delve]),
        "Convoke Phyrexian" => helped_creature(name, &phyrexian, &[Keyword::Convoke]),
        "Delve Phyrexian" => helped_creature(name, &phyrexian, &[Keyword::Delve]),
        "Both Hybrid" => helped_creature(name, &hybrid, &[Keyword::Convoke, Keyword::Delve]),
        "Helped Blaze" => helped_blaze(name, &[Keyword::Convoke, Keyword::Delve])?,
        "Convoke Blaze" => helped_blaze(name, &[Keyword::Convoke])?,
        "Delve Blaze" => helped_blaze(name, &[Keyword::Delve])?,
        other => corpus::card(other)?,
    })
}

/// A creature of one colour that costs one mana of it: a creature to tap, and a card for a graveyard.
fn fodder(slot: usize, color: Color) -> Arc<CardDefinition> {
    let power = i32::try_from(1 + slot % 3).unwrap_or(1);
    creature(&format!("Fodder {slot}"), 0, &[color], power, power)
}

/// A legal Normal deck of lands, spells with convoke and delve (corpus cards, hybrid and Phyrexian costs, an X
/// spell), burn and creatures to tap and to fill graveyards, derived entirely from `rng`.
pub fn help_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [
        Color::Red,
        Color::Black,
        Color::Blue,
        Color::Green,
        Color::White,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..22).map(|i| land(colors[i % 5])).collect();
    let mut cards: Vec<Arc<CardDefinition>> = [
        "Stoke the Flames",
        "Siege Wurm",
        "Treasure Cruise",
        "Murderous Cut",
        "Gurmag Angler",
        "Convoke Hybrid",
        "Delve Hybrid",
        "Convoke Phyrexian",
        "Delve Phyrexian",
        "Both Hybrid",
        "Helped Blaze",
        "Convoke Blaze",
        "Delve Blaze",
    ]
    .iter()
    .filter_map(|name| helped_card(name).ok())
    .collect();
    cards.push(burn("Bolt", 3));
    cards.push(keyworded_creature(
        "Convoking Elf",
        1,
        &[Color::Green],
        1,
        1,
        &[Keyword::Convoke],
    ));
    cards.push(creature_costing(
        "Plain Bear",
        &[ManaSymbol::Generic(1), ManaSymbol::Colored(Color::Green)],
        2,
        2,
    ));
    for card in cards {
        deck.extend(std::iter::repeat_n(card, 2));
    }
    for slot in 0..8 {
        deck.push(fodder(slot, colors[rng.index(5)]));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`help_deck`]s, fully determined by `seed`.
pub fn help_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| help_deck(&mut GameRng::substream(seed, 23_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
