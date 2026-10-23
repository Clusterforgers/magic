//! Cards with a companion ability, and a seeded deck that meets them, for scenarios and fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::companion::DeckCondition;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_cards::value::{Amount, Cmp, StatKind};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, burn, creature, elf, land};

/// Every permanent card type.
fn permanent() -> Pred {
    Pred::AnyOf(
        [
            CardType::Artifact,
            CardType::Creature,
            CardType::Enchantment,
            CardType::Land,
            CardType::Planeswalker,
            CardType::Battle,
        ]
        .map(Pred::Type)
        .to_vec(),
    )
}

fn value(op: Cmp, to: i32) -> Pred {
    Pred::Compare {
        stat: StatKind::ManaValue,
        op,
        to: Amount::Fixed(to),
    }
}

/// "Each permanent card in your starting deck has mana value 2 or less."
pub fn small_permanents() -> DeckCondition {
    DeckCondition::Each {
        of: Selector(vec![permanent()]),
        has: Selector(vec![value(Cmp::Le, 2)]),
    }
}

/// "Your starting deck contains only cards with mana value 3 or greater and land cards."
pub fn lands_and_big_spells() -> DeckCondition {
    DeckCondition::Each {
        of: Selector::default(),
        has: Selector(vec![Pred::AnyOf(vec![
            Pred::Type(CardType::Land),
            value(Cmp::Ge, 3),
        ])]),
    }
}

/// "Each nonland card in your starting deck has a different name."
pub fn distinct_nonlands() -> DeckCondition {
    DeckCondition::Distinct {
        of: Selector(vec![Pred::Not(Box::new(Pred::Type(CardType::Land)))]),
    }
}

/// A creature costing `{1}{G}` whose only text is a companion ability with this condition.
pub fn companion_with(name: &str, condition: DeckCondition) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(crate::cards::cost(1, &[Color::Green]))
            .with_power_toughness(2, 2)
            .with_rules(Ability::Keyword(Keyword::companion(condition))),
    )
}

/// A companion for decks of small permanents (the shape of Lurrus of the Dream-Den's condition).
pub fn small_companion() -> Arc<CardDefinition> {
    companion_with("Small Companion", small_permanents())
}

/// A legal Normal deck of 60 cards whose permanents all have mana value 2 or less, derived entirely from `rng`.
pub fn small_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Green, Color::Red, Color::Blue];
    let mut deck: Vec<Arc<CardDefinition>> = (0..32).map(|i| land(colors[i % 3])).collect();
    deck.extend(std::iter::repeat_n(bear(), 4));
    deck.extend(std::iter::repeat_n(elf(), 4));
    for n in 0..4 {
        deck.extend(std::iter::repeat_n(
            creature(&format!("Little {n}"), 1, &[], 1, 1),
            4,
        ));
    }
    deck.extend(std::iter::repeat_n(burn("Spark", 2), 4));
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`small_deck`]s and whose companion is a [`small_companion`] at each
/// seat, fully determined by `seed`; the decks are checked against the companions when the game begins.
pub fn companion_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| small_deck(&mut GameRng::substream(seed, 70_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
        .with_companions((0..players).map(|_| Some(small_companion())).collect())
}
