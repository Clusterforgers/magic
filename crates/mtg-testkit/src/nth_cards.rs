//! Creatures that wait for the first time in a turn that they attack, are targeted or are dealt damage, and the decks and
//! games built from them and from Seedglaive Mentor, with spells and abilities that target and burn.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, creature, free_creature, land, pinger};
use crate::corpus::{self, CorpusError};
use crate::pump_cards::{gust, rush, shrink};

/// Lands in an nth deck: red and white in turn.
const LANDS: usize = 24;

/// The names of the synthetic creatures of an nth deck.
pub const NTH_CARDS: [&str; 4] = ["Nth Brute", "Nth Bulwark", "Nth Magnet", "Nth Rival"];

/// The corpus creature of an nth deck.
pub const MENTOR: &str = "Seedglaive Mentor";

fn rules(json: &str) -> Result<Ability, CorpusError> {
    serde_json::from_str(json).map_err(|error| CorpusError::Data(format!("{json}: {error}")))
}

/// A creature of this cost and size with one triggered ability that waits for `when` and gains its controller 1 life, or
/// puts a +1/+1 counter on it when `grows`.
fn counting(
    name: &str,
    (generic, color): (u32, Color),
    (power, toughness): (i32, i32),
    (when, grows): (&str, bool),
) -> Result<Arc<CardDefinition>, CorpusError> {
    let effect = if grows {
        r#"{"PutCounters":{"on":"This","counter":"+1/+1"}}"#
    } else {
        r#"{"GainLife":{"amount":1}}"#
    };
    let ability = rules(&format!(
        r#"{{"Triggered":{{"when":{when},"effects":[{effect}]}}}}"#
    ))?;
    Ok(Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(generic, &[color]))
            .with_power_toughness(power, toughness)
            .with_rules(ability),
    ))
}

/// The synthetic creatures: one that grows on its first attack of a turn, one that gains life when first dealt damage in a
/// turn, one for the first time a creature of its controller is targeted by what its controller controls, and one for the
/// second time a creature is targeted by what anyone controls.
pub fn nth_creatures() -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let mine = r#"{"Nth":{"event":{"BecomesTarget":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"by":[{"ControlledBy":"You"}]}},"nth":1}}"#;
    let anyones = r#"{"Nth":{"event":{"BecomesTarget":{"what":[{"Type":"Creature"}]}},"nth":2}}"#;
    Ok(vec![
        counting(
            NTH_CARDS[0],
            (0, Color::Red),
            (1, 1),
            (
                r#"{"Nth":{"event":{"Attacks":{"what":["This"]}},"nth":1}}"#,
                true,
            ),
        )?,
        counting(
            NTH_CARDS[1],
            (1, Color::White),
            (1, 3),
            (
                r#"{"Nth":{"event":{"DealsDamage":{"source":[],"to":"This"}},"nth":1}}"#,
                false,
            ),
        )?,
        counting(NTH_CARDS[2], (1, Color::Red), (2, 1), (mine, false))?,
        counting(NTH_CARDS[3], (1, Color::White), (2, 2), (anyones, false))?,
    ])
}

/// A legal Normal deck in red and white: the corpus Mentor and Lightning Bolt, the synthetic counting creatures, spells and
/// a creature ability that target, and bodies to attack, block and be targeted.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn nth_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Red, Color::White];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 2])).collect();
    deck.extend(std::iter::repeat_n(corpus::card(MENTOR)?, 4));
    deck.extend(std::iter::repeat_n(corpus::card("Lightning Bolt")?, 3));
    for card in nth_creatures()? {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    for (card, copies) in [
        (rush(), 4),
        (gust(), 3),
        (shrink("Nth Shrivel", 1), 2),
        (pinger("Nth Pinger", 1), 3),
        (creature("Nth Bear", 1, &[Color::White], 2, 2), 3),
        (free_creature("Nth Mite", 1, 1), 2),
    ] {
        deck.extend(std::iter::repeat_n(card, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`nth_deck`]s, fully determined by `seed`.
pub fn nth_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(nth_deck(&mut GameRng::substream(seed, 23_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: the synthetic counting creatures use only what the engine runs, and an nth deck is a legal sixty cards.
    use mtg_cards::validate::used_primitives;
    use mtg_engine::caps;

    use super::*;

    #[test]
    fn every_counting_creature_uses_primitives_the_engine_runs() {
        let caps = caps();
        for card in nth_creatures().unwrap() {
            for ability in &card.rules {
                for name in used_primitives(ability) {
                    assert!(caps.runs(&name), "{}: {name}", card.name);
                }
            }
        }
    }

    #[test]
    fn an_nth_deck_has_sixty_cards() {
        let deck = nth_deck(&mut GameRng::substream(1, 1)).unwrap();
        assert_eq!(deck.len(), 60);
    }
}
