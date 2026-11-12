//! Goal: the corpus companions whose condition reads the parity of mana values or a shared card type (Gyruda, Obosh, Umori) are accepted for a 60-card deck exactly when every card they speak of fits, and refused naming the first card that breaks the condition.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::error::SetupError;
use mtg_engine::format::{DeckError, Normal};
use mtg_testkit::cards::{cost, creature, land};
use mtg_testkit::corpus::card;
use rstest::rstest;

type Deck = Vec<Arc<CardDefinition>>;

/// 30 lands and 30 cards of different names made by `spell`.
fn deck(spell: fn(&str) -> Arc<CardDefinition>) -> Deck {
    let mut cards: Deck = (0..30).map(|_| land(Color::Green)).collect();
    cards.extend((0..30).map(|at| spell(&format!("Card {at}"))));
    cards
}

fn two_drop(name: &str) -> Arc<CardDefinition> {
    creature(name, 2, &[], 2, 2)
}

fn three_drop(name: &str) -> Arc<CardDefinition> {
    creature(name, 3, &[], 3, 3)
}

fn artifact_creature() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Construct")
            .with_type(CardType::Artifact)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(3, &[]))
            .with_power_toughness(2, 2),
    )
}

fn instant() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Trick")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(1, &[Color::Green])),
    )
}

/// The deck with one card replaced by `odd_one`.
fn with(mut cards: Deck, odd_one: Arc<CardDefinition>) -> Deck {
    cards[59] = odd_one;
    cards
}

#[rstest]
#[case::gyruda_with_two_drops_and_lands("Gyruda, Doom of Depths", deck(two_drop), None)]
#[case::gyruda_with_a_three_drop(
    "Gyruda, Doom of Depths",
    with(deck(two_drop), three_drop("Three")),
    Some("Three")
)]
#[case::obosh_with_three_drops_and_lands("Obosh, the Preypiercer", deck(three_drop), None)]
#[case::obosh_with_a_two_drop(
    "Obosh, the Preypiercer",
    with(deck(three_drop), two_drop("Two")),
    Some("Two")
)]
#[case::umori_with_creatures_and_artifact_creatures(
    "Umori, the Collector",
    with(deck(two_drop), artifact_creature()),
    None
)]
#[case::umori_with_an_instant_among_creatures(
    "Umori, the Collector",
    with(deck(two_drop), instant()),
    Some("Trick")
)]
fn the_companion_condition_is_checked_against_the_deck(
    #[case] name: &str,
    #[case] cards: Deck,
    #[case] refused_for: Option<&str>,
) {
    let companion = card(name).unwrap();
    let other = deck(two_drop);
    let result = GameSetup::new(Arc::new(Normal), 1, vec![cards, other])
        .with_companions(vec![Some(companion), None])
        .validate();
    match (result, refused_for) {
        (Ok(()), None) => {}
        (
            Err(SetupError::InvalidCompanion {
                source: DeckError::CompanionCondition { reason, .. },
                ..
            }),
            Some(culprit),
        ) => assert!(reason.contains(culprit), "{reason}"),
        (other, expected) => panic!("expected {expected:?}, got {other:?}"),
    }
}
