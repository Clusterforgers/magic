//! Synthetic cards that ask a player to pay mana ("unless its controller pays", "you may pay") or cost mana to
//! activate, and a seeded deck of them with the corpus's Izzet Charm and Rhystic Study for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::Effect;
use mtg_cards::mana::ManaCostX;
use mtg_cards::refs::{Pred, Ref, Selector, Targetable, ZoneKind};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{cost, creature, damage, draw, gain_life, land, one_target};
use crate::corpus::{self, CorpusError};

/// A cost of `{generic}` and one `{colour}` for each of `colored`.
pub fn price(generic: u32, colored: &[Color]) -> Cost {
    Cost::mana(ManaCostX::new(0, cost(generic, colored)))
}

fn payment(who: Ref, price: Cost, paid: Vec<Effect>, unpaid: Vec<Effect>) -> Effect {
    Effect::Pay {
        who,
        cost: price,
        paid,
        unpaid,
    }
}

fn instant(name: &str, generic: u32, colored: &[Color], rules: Ability) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(generic, colored))
            .with_rules(rules),
    )
}

/// An instant for `{1}{U}` aimed at a spell: "Counter target spell unless its controller pays `{tax}`."
pub fn leak(name: &str, tax: u32) -> Arc<CardDefinition> {
    instant(
        name,
        1,
        &[Color::Blue],
        Ability::Spell {
            targets: vec![one_target(Targetable::Object(Selector(vec![Pred::Spell])))],
            effects: vec![payment(
                Ref::ControllerOf(Box::new(Ref::Target(0))),
                price(tax, &[]),
                Vec::new(),
                vec![Effect::Counter {
                    what: Ref::Target(0),
                }],
            )],
        },
    )
}

/// An instant for `{U}`: "Unless an opponent pays `{1}`, you draw a card; if one does, you gain 1 life." With several
/// opponents they are asked in turn order until one pays.
pub fn toll(name: &str) -> Arc<CardDefinition> {
    instant(
        name,
        0,
        &[Color::Blue],
        Ability::Spell {
            targets: Vec::new(),
            effects: vec![payment(
                Ref::Opponents,
                price(1, &[]),
                vec![gain_life(1)],
                vec![draw(1)],
            )],
        },
    )
}

/// An instant for `{W}`: "You may pay `{1}{W}`. If you do, you gain 3 life."
pub fn tithe(name: &str) -> Arc<CardDefinition> {
    instant(
        name,
        0,
        &[Color::White],
        Ability::Spell {
            targets: Vec::new(),
            effects: vec![payment(
                Ref::You,
                price(1, &[Color::White]),
                vec![gain_life(3)],
                Vec::new(),
            )],
        },
    )
}

/// An artifact for `{1}` with an ability that costs `activation` and makes its controller gain 2 life.
pub fn fountain(name: &str, activation: Cost) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(1, &[]))
            .with_rules(Ability::Activated {
                cost: activation,
                timing: Timing::Instant,
                limit: None,
                from: ZoneKind::Battlefield,
                targets: Vec::new(),
                effects: vec![gain_life(2)],
            }),
    )
}

/// A 1/1 creature for `{1}{R}` with "`{1}`, `{T}`: deals 1 damage to any target".
pub fn mana_pinger(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(1, &[Color::Red]))
            .with_power_toughness(1, 1)
            .with_rules(Ability::Activated {
                cost: Cost(vec![
                    CostItem::Mana(ManaCostX::new(0, cost(1, &[]))),
                    CostItem::Tap,
                ]),
                timing: Timing::Instant,
                limit: None,
                from: ZoneKind::Battlefield,
                targets: vec![one_target(Targetable::AnyTarget)],
                effects: vec![damage(Ref::Target(0), 1)],
            }),
    )
}

/// A deck of lands in the three colours, Izzet Charm and Rhystic Study from the corpus, Lightning Bolts to aim them at,
/// spells that ask for a payment, permanents with abilities that cost mana, and creatures, derived entirely from `rng`.
pub fn pay_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Blue, Color::Red, Color::White];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 3])).collect();
    for (name, copies) in [
        ("Izzet Charm", 4),
        ("Rhystic Study", 3),
        ("Lightning Bolt", 4),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    for (card, copies) in [
        (leak("Pay Leak", 2), 3),
        (toll("Pay Toll"), 3),
        (tithe("Pay Tithe"), 3),
        (fountain("Pay Fountain", price(1, &[Color::Blue])), 2),
        (fountain("Pay Chalice", Cost(vec![CostItem::Tap])), 1),
        (mana_pinger("Pay Pinger"), 3),
    ] {
        deck.extend(std::iter::repeat_n(card, copies));
    }
    let mut slot = 0;
    while deck.len() < 60 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let generic = u32::try_from(rng.index(3)).unwrap_or(0);
        let name = format!("Pay Fodder {slot} {generic} {power}/{power}");
        let card = creature(&name, generic, &[colors[rng.index(3)]], power, power);
        let copies = (60 - deck.len()).min(2);
        deck.extend(std::iter::repeat_n(card, copies));
        slot += 1;
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`pay_deck`]s, fully determined by `seed`.
pub fn pay_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(pay_deck(&mut GameRng::substream(seed, 11_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}
