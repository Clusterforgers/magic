//! Cards the engine does not run whole and a seeded deck of them, for the manual actions and their fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::support::Support;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, cost, land};
use crate::corpus::{self, CorpusError};

/// Why the cards below are left to the table.
const REASON: &str = "the engine does not run this clause";

/// A sorcery for `{1}` whose one clause is left to the table: it is cast and goes to the graveyard doing nothing.
pub fn manual_sorcery(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(1, &[]))
            .with_manual("Destroy target creature.", REASON),
    )
}

/// An instant for `{1}` whose one clause is left to the table.
pub fn manual_instant(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(1, &[]))
            .with_manual("Target player loses 2 life.", REASON),
    )
}

/// A 2/2 for `{1}` with flying, which the engine runs, and a triggered clause it does not.
pub fn manual_creature(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(1, &[]))
            .with_power_toughness(2, 2)
            .with_rules(Ability::Keyword(Keyword::Flying))
            .with_manual(
                "Whenever this creature dies, each opponent loses 1 life.",
                REASON,
            ),
    )
}

/// An Aura for `{1}` whose enchant ability and effect are left to the table.
pub fn manual_aura(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Enchantment)
            .with_subtype("Aura")
            .with_mana_cost(cost(1, &[]))
            .with_manual("Enchant creature\nEnchanted creature gets +1/+1.", REASON),
    )
}

/// An Equipment for `{1}` whose static ability and equip ability are left to the table.
pub fn manual_equipment(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_subtype("Equipment")
            .with_mana_cost(cost(1, &[]))
            .with_manual("Equipped creature gets +1/+1.\nEquip {1}", REASON),
    )
}

/// The names of the corpus cards the engine does not play whole today.
pub fn unverified_corpus_cards() -> Result<Vec<String>, CorpusError> {
    let corpus = corpus::corpus()?;
    corpus
        .names()
        .map(|name| corpus.support(name).map(|record| (name, record.support)))
        .filter(|found| !matches!(found, Ok((_, Support::Verified))))
        .map(|found| found.map(|(name, _)| name.to_owned()))
        .collect()
}

/// A legal Normal deck of lands, four of each card the engine does not play whole and of the cards of this module, and a
/// few cards that it does, derived entirely from `rng`.
pub fn manual_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ];
    let mut cards: Vec<Arc<CardDefinition>> = unverified_corpus_cards()?
        .iter()
        .map(|name| corpus::card(name))
        .collect::<Result<_, _>>()?;
    cards.extend([
        manual_sorcery("Hand Wave"),
        manual_instant("Hand Drain"),
        manual_creature("Hand Drake"),
        manual_aura("Hand Charm"),
        manual_equipment("Hand Blade"),
        bear(),
    ]);
    let lands = 60usize.saturating_sub(3 * cards.len());
    let mut deck: Vec<Arc<CardDefinition>> = (0..lands).map(|i| land(colors[i % 5])).collect();
    for card in cards {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`manual_deck`]s, fully determined by `seed`.
pub fn manual_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let decks = (0u64..)
        .take(players)
        .map(|seat| manual_deck(&mut GameRng::substream(seed, 80_000 + seat)))
        .collect::<Result<_, _>>()?;
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}
