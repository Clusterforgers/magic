//! Synthetic cards whose announcement divides an amount among several targets (CR 601.2d): a spell, a mode of a charm,
//! an activated ability and a triggered ability, and a seeded deck of them for fuzz games.
//!
//! The engine does not read a division when the effect runs yet, so each effect deals 1 damage to every target; the
//! announcement is what these cards exercise.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::{Effect, Mode};
use mtg_cards::refs::{Bounds, Ref, TargetSpec, Targetable, ZoneKind};
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, creature, damage, draw, land};
use crate::trigger_cards::trigger;
use crate::trigger_cards::when;

/// One to `most` targets of any kind, sharing `most` to divide among them.
pub fn divided(most: i32) -> TargetSpec {
    TargetSpec {
        what: Targetable::AnyTarget,
        bounds: Bounds {
            min: 1,
            max: Some(Amount::Fixed(most)),
        },
        divide: Some(Amount::Fixed(most)),
    }
}

fn instant(name: &str, rules: Ability) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(1, &[Color::Red]))
            .with_rules(rules),
    )
}

/// An instant for `{1}{R}`: `most` damage divided as you choose among up to `most` targets.
pub fn split_bolt(name: &str, most: i32) -> Arc<CardDefinition> {
    instant(
        name,
        Ability::Spell {
            targets: vec![divided(most)],
            effects: vec![damage(Ref::Target(0), 1)],
        },
    )
}

/// An instant for `{1}{R}`: choose one, damage divided among up to two targets or draw a card.
pub fn split_charm(name: &str) -> Arc<CardDefinition> {
    instant(
        name,
        Ability::Spell {
            targets: Vec::new(),
            effects: vec![Effect::Modal {
                choose: Bounds {
                    min: 1,
                    max: Some(Amount::Fixed(1)),
                },
                repeat: false,
                modes: vec![
                    Mode {
                        label: Some("Split".to_owned()),
                        targets: vec![divided(2)],
                        effects: vec![damage(Ref::Target(0), 1)],
                    },
                    Mode {
                        label: Some("Cards".to_owned()),
                        targets: Vec::new(),
                        effects: vec![draw(1)],
                    },
                ],
            }],
        },
    )
}

/// A 1/1 creature for `{R}` with "{T}: 2 damage divided as you choose among up to two targets".
pub fn split_pinger(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_power_toughness(1, 1)
            .with_rules(Ability::Activated {
                cost: Cost(vec![CostItem::Tap]),
                timing: Timing::Instant,
                condition: None,
                limit: None,
                from: ZoneKind::Battlefield,
                targets: vec![divided(2)],
                effects: vec![damage(Ref::Target(0), 1)],
            }),
    )
}

/// A 2/2 creature for `{1}{R}` with "when this enters, 2 damage divided as you choose among up to two targets".
pub fn split_hawk(name: &str) -> Arc<CardDefinition> {
    let mut ability = trigger(when::this_enters(), vec![damage(Ref::Target(0), 1)]);
    if let Ability::Triggered { targets, .. } = &mut ability {
        targets.push(divided(2));
    }
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(1, &[Color::Red]))
            .with_power_toughness(2, 2)
            .with_rules(ability),
    )
}

/// A legal Normal deck of lands, the four split cards and creatures to aim them at, derived entirely from `rng`.
pub fn split_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Red, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 2])).collect();
    for card in [
        split_bolt("Split Bolt", 3),
        split_bolt("Split Spark", 2),
        split_charm("Split Charm"),
        split_pinger("Split Pinger"),
        split_hawk("Split Hawk"),
    ] {
        deck.extend(std::iter::repeat_n(card, 4));
    }
    for slot in 0..8 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Split Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[colors[rng.index(2)]], power, power);
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`split_deck`]s, fully determined by `seed`.
pub fn split_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| split_deck(&mut GameRng::substream(seed, 13_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
