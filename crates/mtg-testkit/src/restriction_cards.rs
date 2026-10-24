//! Abilities that may be activated only in a moment of the turn or under a count: cards that carry a restriction as
//! data, and the decks and games built from them with the corpus cards that read "Activate only during your upkeep", "...
//! during your turn, before attackers are declared", "... during the end of combat step" and "... only if there are at
//! least three Zombie creature cards in your graveyard".

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a restriction deck: black, white and red in turn.
const LANDS: usize = 24;

/// An artifact with no cost whose `{T}` ability draws a card when `condition` (data) holds.
pub fn moment_source(name: &str, condition: &str) -> Result<Arc<CardDefinition>, CorpusError> {
    let json = format!(
        r#"{{"Activated":{{"cost":["Tap"],"condition":{condition},"effects":[{{"Draw":{{}}}}]}}}}"#
    );
    let ability: Ability = serde_json::from_str(&json)
        .map_err(|error| CorpusError::Data(format!("{json}: {error}")))?;
    Ok(Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability),
    ))
}

/// A legal Normal deck in black, white and red of the cards that may be activated only at some time (Eternal Dragon,
/// Tomb Tyrant, Hua Tuo, Loyal Retainers, Desert, Vona, Hoofprints of the Stag, Wolfwillow Haven and three sources of
/// the moments of combat), with Zombies to fill a graveyard and creatures to sacrifice, derived entirely from `rng`.
pub fn restriction_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Black, Color::White, Color::Red];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 3])).collect();
    for (name, copies) in [
        ("Eternal Dragon", 2),
        ("Tomb Tyrant", 3),
        ("Hua Tuo, Honored Physician", 2),
        ("Loyal Retainers", 2),
        ("Desert", 2),
        ("Vona, Butcher of Magan", 1),
        ("Hoofprints of the Stag", 1),
        ("Wolfwillow Haven", 1),
        ("Lightning Bolt", 4),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (name, condition) in [
        ("Combat Source", r#""InCombat""#),
        ("Upkeep Source", r#"{"All":["YourTurn",{"Step":"Upkeep"}]}"#),
        ("Early Source", r#"{"All":["YourTurn","BeforeAttackers"]}"#),
    ] {
        deck.push(moment_source(name, condition)?);
    }
    for slot in 0..(60 - deck.len()) {
        let generic = u32::try_from(rng.index(3)).unwrap_or(0);
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Restriction Fodder {slot} {generic} {power}/{power}");
        deck.push(creature(
            &name,
            generic,
            &[colors[rng.index(3)]],
            power,
            power,
        ));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`restriction_deck`]s, fully determined by `seed`.
pub fn restriction_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(restriction_deck(&mut GameRng::substream(
            seed,
            13_000 + seat,
        ))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}
