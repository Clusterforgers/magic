//! Decks for the games that exercise copies: the cards that enter as a copy, make token copies and populate, with
//! creatures to copy, token makers to populate from and spells to aim at the copies.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus::{self, CorpusError};

/// A deck of lands in three colours, Clone, Phantasmal Image, Mirror Image, Cackling Counterpart, Rootborn Defenses,
/// Growing Ranks, Followed Footsteps, Second Harvest, Rhys the Redeemed, Auton Soldier, Malleable Impostor, Imposter
/// Mech and Machine God's Effigy from the corpus, Hero of Bladehold and Questing Beast to copy and populate from,
/// Lightning Bolts to aim at them, and creatures, derived entirely from `rng`.
pub fn copy_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    copies_deck(
        rng,
        22,
        &[
            ("Clone", 2),
            ("Phantasmal Image", 2),
            ("Mirror Image", 2),
            ("Cackling Counterpart", 3),
            ("Rootborn Defenses", 2),
            ("Growing Ranks", 2),
            ("Followed Footsteps", 2),
            ("Second Harvest", 2),
            ("Rhys the Redeemed", 1),
            ("Auton Soldier", 2),
            ("Malleable Impostor", 2),
            ("Imposter Mech", 2),
            ("Machine God's Effigy", 2),
            ("Hero of Bladehold", 3),
            ("Lightning Bolt", 3),
        ],
    )
}

/// A deck of `lands` lands in three colours and the corpus cards `cards` (name and count), filled up with creatures
/// to copy, derived entirely from `rng`.
fn copies_deck(
    rng: &mut GameRng,
    lands: usize,
    cards: &[(&str, usize)],
) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Blue, Color::White, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..lands).map(|i| land(colors[i % 3])).collect();
    for (name, copies) in cards {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, *copies));
    }
    let mut slot = 0;
    while deck.len() < 60 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let generic = u32::try_from(rng.index(3)).unwrap_or(0);
        let name = format!("Copy Fodder {slot} {generic} {power}/{power}");
        let card = creature(&name, generic, &[colors[rng.index(3)]], power, power);
        let copies = (60 - deck.len()).min(2);
        deck.extend(std::iter::repeat_n(card, copies));
        slot += 1;
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`copy_deck`]s, fully determined by `seed`.
pub fn copy_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(copy_deck(&mut GameRng::substream(seed, 12_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

/// A deck heavy in the permanents that enter as a copy with exceptions (Auton Soldier, Malleable Impostor, Imposter
/// Mech, Machine God's Effigy) and the cards that copy every token (Second Harvest, Rhys the Redeemed), with token
/// makers and creatures to copy, derived entirely from `rng`.
pub fn copy_shapes_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    copies_deck(
        rng,
        20,
        &[
            ("Auton Soldier", 3),
            ("Malleable Impostor", 4),
            ("Imposter Mech", 4),
            ("Machine God's Effigy", 3),
            ("Second Harvest", 3),
            ("Rhys the Redeemed", 1),
            ("Cackling Counterpart", 4),
            ("Hero of Bladehold", 4),
            ("Lightning Bolt", 2),
        ],
    )
}

/// A normal game of `players` seats whose decks are [`copy_shapes_deck`]s, fully determined by `seed`.
pub fn copy_shapes_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(copy_shapes_deck(&mut GameRng::substream(
            seed,
            12_500 + seat,
        ))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

/// A deck of lands in four colours, the cards that copy spells and abilities (Twincast, Reverberate, Fork, Reiterate,
/// Dualcaster Mage, Strionic Resonator, Lithoform Engine, Cytoshape), the storm spells (Grapeshot, Brain Freeze, Tendrils of
/// Agony), spells to copy (Lightning Bolt, Burst Lightning) and creatures with triggers to copy, derived entirely from `rng`.
pub fn spell_copy_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Red, Color::Blue, Color::Black, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..22).map(|i| land(colors[i % 4])).collect();
    for (name, copies) in [
        ("Twincast", 3),
        ("Reverberate", 2),
        ("Fork", 2),
        ("Reiterate", 2),
        ("Grapeshot", 3),
        ("Brain Freeze", 2),
        ("Tendrils of Agony", 2),
        ("Strionic Resonator", 2),
        ("Lithoform Engine", 1),
        ("Dualcaster Mage", 2),
        ("Cytoshape", 2),
        ("Lightning Bolt", 4),
        ("Burst Lightning", 3),
        ("Armament Corps", 1),
        ("Hero of Bladehold", 2),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    let mut slot = 0;
    while deck.len() < 60 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Copy Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[colors[rng.index(4)]], power, power);
        let copies = (60 - deck.len()).min(2);
        deck.extend(std::iter::repeat_n(card, copies));
        slot += 1;
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`spell_copy_deck`]s, fully determined by `seed`.
pub fn spell_copy_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(spell_copy_deck(&mut GameRng::substream(
            seed,
            13_000 + seat,
        ))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}
