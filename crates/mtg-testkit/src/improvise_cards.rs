//! Cards for improvise, affinity and typecycling beside the corpus cards that have the keywords, and a seeded deck of
//! them for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::Color;
use mtg_core::mana::{ManaCost, ManaSymbol};
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, creature, keyworded_creature, land};
use crate::corpus::{self, CorpusError};
use crate::help_cards::{helped_blaze, helped_creature};

fn cost_of(text: &str) -> Result<ManaCost, CorpusError> {
    ManaCost::parse(text).map_err(|error| CorpusError::Data(error.to_string()))
}

/// An artifact that costs `{1}` and does nothing: a permanent to improvise with.
fn trinket(name: &str) -> Result<Arc<CardDefinition>, CorpusError> {
    Ok(Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_mana_cost(cost_of("{1}")?),
    ))
}

/// An artifact creature that costs nothing: it improvises, convokes and counts for affinity.
fn clockwork(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_type(CardType::Creature)
            .with_power_toughness(0, 2),
    )
}

/// A permanent of the kind affinity counts, with no cost: a legendary creature or artifact, a Saga or an Equipment; a plain
/// enchantment is none of them.
fn historic(name: &str) -> Option<Arc<CardDefinition>> {
    let card = CardDefinition::new(name);
    Some(Arc::new(match name {
        "Legend" => card
            .with_type(CardType::Creature)
            .with_supertype(Supertype::Legendary)
            .with_power_toughness(1, 1),
        "Chapter" => card.with_type(CardType::Enchantment).with_subtype("Saga"),
        "Blade" => card.with_type(CardType::Artifact).with_subtype("Equipment"),
        "Legendary Relic" => card
            .with_type(CardType::Artifact)
            .with_supertype(Supertype::Legendary),
        "Plain Enchantment" => card.with_type(CardType::Enchantment),
        _ => return None,
    }))
}

/// A creature with a cost and these keywords, of any colour.
fn with_keywords(
    name: &str,
    cost: &str,
    keywords: Vec<Keyword>,
) -> Result<Arc<CardDefinition>, CorpusError> {
    let base = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_mana_cost(cost_of(cost)?)
        .with_power_toughness(2, 2);
    Ok(Arc::new(
        keywords.into_iter().fold(base, |card, keyword| {
            card.with_rules(Ability::Keyword(keyword))
        }),
    ))
}

/// Affinity for the permanents of one card type.
pub fn affinity_for(card_type: CardType) -> Keyword {
    Keyword::Affinity(Selector(vec![Pred::Type(card_type)]))
}

/// Typecycling that finds the basic land cards, or the land type given, for `cost`.
pub fn landcycling(land_type: Option<&str>, cost: &str) -> Result<Keyword, CorpusError> {
    let find = match land_type {
        Some(name) => vec![Pred::Subtype(Subtype(name.to_owned()))],
        None => vec![
            Pred::Type(CardType::Land),
            Pred::Supertype(Supertype::Basic),
        ],
    };
    Ok(Keyword::Typecycling {
        find: Selector(find),
        cost: Cost::mana(
            cost.parse()
                .map_err(|error: mtg_core::mana::ParseManaCostError| {
                    CorpusError::Data(error.to_string())
                })?,
        ),
    })
}

/// The cards of this module and the corpus cards with improvise, affinity or typecycling, by name.
pub fn improvise_card(name: &str) -> Result<Arc<CardDefinition>, CorpusError> {
    let blue = [ManaSymbol::Generic(2), ManaSymbol::Colored(Color::Blue)];
    Ok(match name {
        "Trinket" | "Gadget" | "Gizmo" => trinket(name)?,
        "Clockwork" => clockwork(name),
        "Improvise Bear" => helped_creature(name, &blue, &[Keyword::Improvise]),
        "Improvise Convoke Bear" => {
            helped_creature(name, &blue, &[Keyword::Improvise, Keyword::Convoke])
        }
        "Improvise Delve Bear" => {
            helped_creature(name, &blue, &[Keyword::Improvise, Keyword::Delve])
        }
        "Improvise Blaze" => helped_blaze(name, &[Keyword::Improvise])?,
        "Affinity Golem" => with_keywords(name, "{6}", vec![affinity_for(CardType::Artifact)])?,
        "Island Sphinx" => with_keywords(
            name,
            "{4}{U}",
            vec![Keyword::Affinity(Selector(vec![Pred::Subtype(Subtype(
                "Island".into(),
            ))]))],
        )?,
        "Improvise Affinity Golem" => with_keywords(
            name,
            "{5}{B}",
            vec![affinity_for(CardType::Artifact), Keyword::Improvise],
        )?,
        "Equipment Golem" => with_keywords(
            name,
            "{5}",
            vec![Keyword::Affinity(Selector(vec![Pred::Subtype(Subtype(
                "Equipment".into(),
            ))]))],
        )?,
        // A spell that is no artifact and one that is: an ability that gives improvise to the first leaves the second.
        "Statuary Bear" => creature(name, 3, &[Color::Blue], 2, 2),
        "Pricey Relic" => Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Artifact)
                .with_mana_cost(cost_of("{4}")?),
        ),
        "Landcycling Ogre" => with_keywords(name, "{3}{G}", vec![landcycling(None, "{1}")?])?,
        "Swampcycling Ogre" => {
            with_keywords(name, "{3}{B}", vec![landcycling(Some("Swamp"), "{B}")?])?
        }
        other => match historic(other) {
            Some(card) => card,
            None => corpus::card(other)?,
        },
    })
}

/// A legal Normal deck of lands, spells and creatures with improvise, affinity and typecycling (corpus cards and
/// synthetic ones), artifacts to tap and count, and burn, derived entirely from `rng`.
pub fn improvise_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
        Color::White,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..22).map(|i| land(colors[i % 5])).collect();
    let mut cards: Vec<Arc<CardDefinition>> = [
        "Reverse Engineer",
        "Thoughtcast",
        "Thought Monitor",
        "Panther Robot",
        "Ash Barrens",
        "Valley Rannet",
        "Chartooth Cougar",
        "Fiery Fall",
        "Sol Ring",
        "Trinket",
        "Gadget",
        "Gizmo",
        "Clockwork",
        "Improvise Bear",
        "Improvise Convoke Bear",
        "Improvise Delve Bear",
        "Improvise Blaze",
        "Affinity Golem",
        "Island Sphinx",
        "Improvise Affinity Golem",
        "Landcycling Ogre",
        "Swampcycling Ogre",
    ]
    .iter()
    .filter_map(|name| improvise_card(name).ok())
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
    cards.push(creature("Plain Bear", 2, &[Color::Green], 2, 2));
    for card in cards {
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A legal Normal deck of the permanents that give improvise and convoke to spells (Inspiring Statuary, Chief Engineer),
/// the spells and helpers they act on, and the cards that count historic permanents and Equipment, derived entirely
/// from `rng`.
pub fn granted_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Blue, Color::White, Color::Red, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..26).map(|i| land(colors[i % 4])).collect();
    let mut cards: Vec<Arc<CardDefinition>> = [
        "Inspiring Statuary",
        "Chief Engineer",
        "Statuary Bear",
        "Pricey Relic",
        "Trinket",
        "Gadget",
        "Clockwork",
        "Sol Ring",
        "Improvise Bear",
        "Banish to Another Universe",
        "Equipment Golem",
        "Legend",
        "Chapter",
        "Blade",
        "Legendary Relic",
    ]
    .iter()
    .filter_map(|name| improvise_card(name).ok())
    .collect();
    cards.push(burn("Bolt", 3));
    cards.push(creature("Plain Bear", 2, &[Color::Green], 2, 2));
    for card in cards {
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`granted_deck`]s, fully determined by `seed`.
pub fn granted_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| granted_deck(&mut GameRng::substream(seed, 24_500 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}

/// A normal game of `players` seats whose decks are [`improvise_deck`]s, fully determined by `seed`.
pub fn improvise_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| improvise_deck(&mut GameRng::substream(seed, 24_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
