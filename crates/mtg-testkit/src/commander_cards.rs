//! Commander games for tests: legendary commanders and pairs of partners, 100-card singleton decks built from the corpus
//! within the color identity of their commanders, and setups of 2 to 6 seats.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::support::Support;
use mtg_core::card::{CardType, Supertype};
use mtg_core::color::{Color, ColorSet};
use mtg_engine::GameSetup;
use mtg_engine::format::Commander;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, land, wastes};
use crate::corpus::{self, CorpusError};

/// Cards of the deck besides its commander.
pub const DECK_CARDS: usize = 99;
/// The most distinct corpus cards one generated deck holds; basic lands fill the rest.
const MOST_SPELLS: usize = 60;

/// A legendary creature costing `generic` and one colored mana per entry of `colors`.
pub fn legend(
    name: &str,
    generic: u32,
    colors: &[Color],
    power: i32,
    toughness: i32,
) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_mana_cost(cost(generic, colors))
            .with_supertype(Supertype::Legendary)
            .with_type(CardType::Creature)
            .with_power_toughness(power, toughness),
    )
}

/// A legendary creature with Partner, costing `generic` and one colored mana per entry of `colors`.
pub fn partner_legend(
    name: &str,
    generic: u32,
    colors: &[Color],
    power: i32,
    toughness: i32,
) -> Arc<CardDefinition> {
    let card = legend(name, generic, colors, power, toughness)
        .as_ref()
        .clone();
    Arc::new(card.with_rules(Ability::Keyword(Keyword::Partner)))
}

/// A basic land of each color of `identity`, or Wastes for a colorless identity.
fn basics(identity: ColorSet) -> Vec<Arc<CardDefinition>> {
    let lands: Vec<_> = identity.iter().map(land).collect();
    if lands.is_empty() {
        vec![wastes()]
    } else {
        lands
    }
}

/// The corpus cards the engine plays whole.
fn playable() -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let corpus = corpus::corpus()?;
    let mut playable = Vec::new();
    for name in corpus.names() {
        if corpus.support(name)?.support == Support::Verified {
            playable.push(corpus.card(name)?);
        }
    }
    Ok(playable)
}

/// `size` cards for the deck of `commanders`: the `required` cards, distinct playable corpus cards within their color
/// identities together, and basic lands to fill.
fn deck_for(
    rng: &mut GameRng,
    playable: Vec<Arc<CardDefinition>>,
    commanders: &[&CardDefinition],
    required: &[Arc<CardDefinition>],
    size: usize,
) -> Vec<Arc<CardDefinition>> {
    let identity = commanders.iter().fold(ColorSet::EMPTY, |all, card| {
        card.color_identity().iter().fold(all, ColorSet::with)
    });
    let mut spells: Vec<_> = playable
        .into_iter()
        .filter(|card| commanders.iter().all(|leader| card.name != leader.name))
        .filter(|card| required.iter().all(|kept| card.name != kept.name))
        .filter(|card| card.color_identity().is_subset_of(identity))
        .filter(|card| card.singleton_limit() == Some(1))
        .collect();
    rng.shuffle(&mut spells);
    spells.truncate(MOST_SPELLS.saturating_sub(required.len()));
    let basics = basics(identity);
    let mut deck = required.to_vec();
    deck.extend(spells);
    let mut next = 0;
    while deck.len() < size {
        deck.push(Arc::clone(&basics[next % basics.len()]));
        next += 1;
    }
    rng.shuffle(&mut deck);
    deck
}

/// A legal Commander deck led by a legendary creature of the corpus: the 99 other cards (distinct corpus cards the
/// engine plays whole, within the commander's color identity, and basic lands to fill), and the commander. Everything
/// is derived from `rng`.
pub fn commander_deck(
    rng: &mut GameRng,
) -> Result<(Vec<Arc<CardDefinition>>, Arc<CardDefinition>), CorpusError> {
    let playable = playable()?;
    let leaders: Vec<_> = playable
        .iter()
        .filter(|card| card.can_be_commander())
        .cloned()
        .collect();
    let commander = leaders
        .get(rng.index(leaders.len().max(1)))
        .cloned()
        .ok_or_else(|| CorpusError::Data("the corpus has no commander".into()))?;
    let deck = deck_for(rng, playable, &[&commander], &[], DECK_CARDS);
    Ok((deck, commander))
}

/// A deck and its pair of partners.
pub type PartnerDeck = (Vec<Arc<CardDefinition>>, [Arc<CardDefinition>; 2]);

/// A legal Commander deck led by a pair of partners in two colors: the 98 other cards, within the two colors together,
/// and the pair. Everything is derived from `rng`.
pub fn partner_deck(rng: &mut GameRng) -> Result<PartnerDeck, CorpusError> {
    let at = rng.index(Color::ALL.len());
    let first = Color::ALL[at];
    let second = Color::ALL[(at + 1 + rng.index(Color::ALL.len() - 1)) % Color::ALL.len()];
    let pair = [
        partner_legend("First Partner", 1, &[first], 2, 2),
        partner_legend("Second Partner", 1, &[second], 2, 2),
    ];
    let deck = deck_for(rng, playable()?, &[&pair[0], &pair[1]], &[], DECK_CARDS - 1);
    Ok((deck, pair))
}

/// A Commander game of `players` seats with [`commander_deck`]s, fully determined by `seed`.
pub fn commander_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    let mut commanders = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        let (deck, commander) = commander_deck(&mut GameRng::substream(seed, 9_000 + seat))?;
        decks.push(deck);
        commanders.push(vec![commander]);
    }
    Ok(GameSetup::new(Arc::new(Commander), seed, decks).with_commanders(commanders))
}

/// The corpus cards that read a cast trigger on a subtype, "permanent", "multicolored" or "historic" spell, the limit
/// "This ability triggers only once each turn." or the times a commander was cast.
pub const CAST_KIND_CARDS: [&str; 9] = [
    "Commander's Insignia",
    "Jyoti, Moag Ancient",
    "Deeproot Waters",
    "Hero of Precinct One",
    "Archmage of Echoes",
    "Sarah Jane Smith",
    "True Identity",
    "Teshar, Ancestor's Apostle",
    "Oyobi, Who Split the Heavens",
];

/// The subtypes of spells the cards of [`CAST_KIND_CARDS`] watch for.
const CAST_KIND_SUBTYPES: [&str; 6] = ["Merfolk", "Faerie", "Wizard", "Spirit", "Arcane", "Saga"];

/// A legal Commander deck in all five colors led by a legend that is cast again and again: the cards of
/// [`CAST_KIND_CARDS`], up to 24 more corpus cards that those watch for (multicolored, artifact and subtype spells),
/// the rest of the corpus within the identity and basic lands. Everything is derived from `rng`.
pub fn cast_kind_deck(
    rng: &mut GameRng,
) -> Result<(Vec<Arc<CardDefinition>>, Arc<CardDefinition>), CorpusError> {
    let commander = legend("Rainbow Leader", 0, &Color::ALL, 3, 3);
    let mut required = CAST_KIND_CARDS
        .iter()
        .map(|name| corpus::card(name))
        .collect::<Result<Vec<_>, _>>()?;
    let mut watched: Vec<_> = playable()?
        .into_iter()
        .filter(|card| !CAST_KIND_CARDS.contains(&card.name.as_str()))
        .filter(|card| {
            card.colors().iter().count() >= 2
                || card.is_type(CardType::Artifact)
                || card
                    .subtypes
                    .iter()
                    .any(|found| CAST_KIND_SUBTYPES.contains(&found.0.as_str()))
        })
        .filter(|card| card.singleton_limit() == Some(1))
        .collect();
    rng.shuffle(&mut watched);
    watched.truncate(24);
    required.extend(watched);
    let deck = deck_for(rng, playable()?, &[&commander], &required, DECK_CARDS);
    Ok((deck, commander))
}

/// A Commander game of `players` seats with [`cast_kind_deck`]s, fully determined by `seed`.
pub fn cast_kind_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    let mut commanders = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        let (deck, commander) = cast_kind_deck(&mut GameRng::substream(seed, 9_800 + seat))?;
        decks.push(deck);
        commanders.push(vec![commander]);
    }
    Ok(GameSetup::new(Arc::new(Commander), seed, decks).with_commanders(commanders))
}

/// A Commander game of `players` seats, each led by a pair of partners ([`partner_deck`]), fully determined by `seed`.
pub fn partner_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    let mut commanders = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        let (deck, pair) = partner_deck(&mut GameRng::substream(seed, 9_500 + seat))?;
        decks.push(deck);
        commanders.push(pair.to_vec());
    }
    Ok(GameSetup::new(Arc::new(Commander), seed, decks).with_commanders(commanders))
}
