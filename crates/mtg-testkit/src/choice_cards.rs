//! Synthetic spells that ask a player something: a "may", a choice among opponents and a charm with modes, and a
//! seeded deck of them for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::{Effect, Mode};
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, Targetable};
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, creature, damage, draw, gain_life, land, one_target};

fn instant(
    name: &str,
    generic: u32,
    colored: &[Color],
    effects: Vec<Effect>,
) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(generic, colored))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects,
            }),
    )
}

/// An instant for `{U}`: "You may draw a card."
pub fn may_draw(name: &str) -> Arc<CardDefinition> {
    instant(
        name,
        0,
        &[Color::Blue],
        vec![Effect::May {
            who: Ref::You,
            effects: vec![draw(1)],
        }],
    )
}

/// An instant for `{U}` aimed at a player: "Target player may draw a card. You gain 2 life."
pub fn gift(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Blue]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::Player)],
                effects: vec![
                    Effect::May {
                        who: Ref::Target(0),
                        effects: vec![Effect::Draw {
                            who: Ref::Target(0),
                            count: Amount::Fixed(1),
                        }],
                    },
                    gain_life(2),
                ],
            }),
    )
}

/// An instant for `{R}`: "Choose an opponent. They lose 3 life."
pub fn scorn(name: &str) -> Arc<CardDefinition> {
    instant(
        name,
        0,
        &[Color::Red],
        vec![
            Effect::Choose {
                who: Ref::You,
                from: Ref::Opponents,
                count: Bounds::default(),
                bind: "victim".into(),
                rest_as: None,
            },
            Effect::LoseLife {
                who: Ref::Bound("victim".into()),
                amount: Amount::Fixed(3),
            },
        ],
    )
}

/// An instant for `{1}{R}` with four modes, of which `min` to `max` are chosen and which may repeat when `repeat`:
/// 0 deals 2 damage to target creature, 1 gains 3 life, 2 draws a card, 3 makes target player lose 2 life.
pub fn charm(name: &str, min: u32, max: u32, repeat: bool) -> Arc<CardDefinition> {
    let mode = |label: &str, targets, effects| Mode {
        label: Some(label.to_owned()),
        targets,
        effects,
    };
    let creature_target = one_target(Targetable::Object(Selector(vec![Pred::Type(
        CardType::Creature,
    )])));
    instant(
        name,
        1,
        &[Color::Red],
        vec![Effect::Modal {
            choose: Bounds {
                min: Amount::Fixed(i32::try_from(min).unwrap_or(i32::MAX)),
                max: Some(Amount::Fixed(i32::try_from(max).unwrap_or(i32::MAX))),
            },
            repeat,
            modes: vec![
                mode(
                    "Damage",
                    vec![creature_target],
                    vec![damage(Ref::Target(0), 2)],
                ),
                mode("Life", Vec::new(), vec![gain_life(3)]),
                mode("Cards", Vec::new(), vec![draw(1)]),
                mode(
                    "Drain",
                    vec![one_target(Targetable::Player)],
                    vec![Effect::LoseLife {
                        who: Ref::Target(0),
                        amount: Amount::Fixed(2),
                    }],
                ),
            ],
        }],
    )
}

/// A deck of lands, optional draws, choices and charms (one, two or up to three modes, one of them repeating) and
/// creatures to aim them at, derived entirely from `rng`.
pub fn choice_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Blue, Color::Red];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 2])).collect();
    for card in [
        may_draw("Choice Study"),
        gift("Choice Gift"),
        scorn("Choice Scorn"),
        charm("Choice Charm", 1, 1, false),
        charm("Choice Double Charm", 2, 2, false),
        charm("Choice Wide Charm", 0, 3, true),
    ] {
        deck.extend(std::iter::repeat_n(card, 4));
    }
    for slot in 0..6 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Choice Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[colors[rng.index(2)]], power, power);
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    deck
}

/// A normal game of `players` seats whose decks are [`choice_deck`]s, fully determined by `seed`.
pub fn choice_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..players as u64)
        .map(|seat| choice_deck(&mut GameRng::substream(seed, 7_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
