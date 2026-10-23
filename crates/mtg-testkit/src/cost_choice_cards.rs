//! A seeded deck of the corpus cards whose required additional cost is a choice ("discard a card or pay 3 life", "reveal a
//! Dinosaur card from your hand or pay {1}"), with swamps, forests, Dinosaurs and fodder to give, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus cards whose required additional cost is a choice between costs.
pub const COST_CHOICE_CARDS: &[&str] = &["Bitter Triumph", "Dusk Mangler", "Thunderherd Migration"];

/// A Dinosaur creature card for {1}{G}.
fn dinosaur(slot: usize) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(format!("Fuzz Dinosaur {slot}"))
            .with_type(CardType::Creature)
            .with_subtype("Dinosaur")
            .with_mana_cost(crate::cards::cost(1, &[Color::Green]))
            .with_power_toughness(2, 2),
    )
}

/// A legal Normal deck of swamps and forests, three of each choice card, Dinosaurs and creatures to sacrifice, discard and
/// destroy, derived entirely from `rng`.
pub fn cost_choice_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let mut deck: Vec<Arc<CardDefinition>> = (0..12).map(|_| land(Color::Black)).collect();
    deck.extend((0..12).map(|_| land(Color::Green)));
    for name in COST_CHOICE_CARDS {
        if let Ok(card) = corpus::card(name) {
            deck.extend(std::iter::repeat_n(card, 3));
        }
    }
    deck.extend((0..3).map(dinosaur));
    for slot in 0..8 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Choice Fodder {slot} {power}/{power}");
        deck.extend(std::iter::repeat_n(
            creature(&name, 1, &[Color::Black], power, power),
            3,
        ));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`cost_choice_deck`]s, fully determined by `seed`.
pub fn cost_choice_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| cost_choice_deck(&mut GameRng::substream(seed, 32_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
