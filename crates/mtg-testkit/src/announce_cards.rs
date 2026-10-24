//! Synthetic cards whose announcement divides an amount among several targets (CR 601.2d): a spell, a mode of a charm,
//! an activated ability and a triggered ability, spells that divide damage evenly or put counters, and a seeded deck of
//! them for fuzz games. Each effect gives every target its share of the division the announcement recorded.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::{Effect, Mode};
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, creature, draw, land};
use crate::trigger_cards::trigger;
use crate::trigger_cards::when;

/// One to `most` targets of any kind, sharing `most` to divide among them.
pub fn divided(most: i32) -> TargetSpec {
    TargetSpec {
        what: Targetable::AnyTarget,
        bounds: Bounds {
            min: Amount::one(),
            max: Some(Amount::Fixed(most)),
        },
        divide: Some(Amount::Fixed(most)),
    }
}

/// Up to `most` creatures, sharing `most` to divide among them.
pub fn divided_creatures(most: i32) -> TargetSpec {
    TargetSpec {
        what: Targetable::Object(Selector(vec![Pred::Type(CardType::Creature)])),
        ..divided(most)
    }
}

/// Each target of the first specification is dealt its share of the division.
fn share_damage() -> Effect {
    Effect::DealDamage {
        source: None,
        to: Ref::Target(0),
        amount: Amount::Divided,
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
            effects: vec![share_damage()],
        },
    )
}

/// An instant for `{1}{R}`: `total` damage divided evenly, rounded down, among any number of targets.
pub fn even_bolt(name: &str, total: i32) -> Arc<CardDefinition> {
    instant(
        name,
        Ability::Spell {
            targets: vec![TargetSpec {
                what: Targetable::AnyTarget,
                bounds: Bounds {
                    min: Amount::one(),
                    max: None,
                },
                divide: None,
            }],
            effects: vec![Effect::DealDamage {
                source: None,
                to: Ref::Target(0),
                amount: Amount::EvenShare(Box::new(Amount::Fixed(total))),
            }],
        },
    )
}

/// A sorcery for `{1}{G}`: `most` +1/+1 counters divided as you choose among up to `most` creatures.
pub fn split_growth(name: &str, most: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(1, &[Color::Green]))
            .with_rules(Ability::Spell {
                targets: vec![divided_creatures(most)],
                effects: vec![Effect::PutCounters {
                    on: Ref::Target(0),
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Divided,
                }],
            }),
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
                    min: Amount::one(),
                    max: Some(Amount::Fixed(1)),
                },
                repeat: false,
                modes: vec![
                    Mode {
                        label: Some("Split".to_owned()),
                        targets: vec![divided(2)],
                        effects: vec![share_damage()],
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
                effects: vec![share_damage()],
            }),
    )
}

/// A 2/2 creature for `{1}{R}` with "when this enters, 2 damage divided as you choose among up to two targets".
pub fn split_hawk(name: &str) -> Arc<CardDefinition> {
    let mut ability = trigger(when::this_enters(), vec![share_damage()]);
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
    let mut deck: Vec<Arc<CardDefinition>> = (0..22).map(|i| land(colors[i % 2])).collect();
    for card in [
        split_bolt("Split Bolt", 3),
        split_bolt("Split Spark", 2),
        even_bolt("Even Bolt", 5),
        split_growth("Split Growth", 3),
        split_charm("Split Charm"),
        split_pinger("Split Pinger"),
        split_hawk("Split Hawk"),
    ] {
        deck.extend(std::iter::repeat_n(card, 4));
    }
    for slot in 0..5 {
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
