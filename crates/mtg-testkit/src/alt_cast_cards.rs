//! The table of alternative costs (`Ability::AltCast`): one row per card, each a creature with the way it can be cast
//! for, shared by the scenario tests and the seeded fuzz decks.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::Effect;
use mtg_cards::mana::ManaCostX;
use mtg_cards::modify::{CastPermission, Duration, GameRule, Modification};
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen, Step, TriggerPattern};
use mtg_cards::value::{Amount, CastWay, Cmp, Condition, CounterKind, TurnTally};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};

/// The name the warped permanent is bound to.
const WARPED: &str = "warped";

/// The name the warped card is bound to once it lies in exile.
const EXILED: &str = "warped card";

/// One row of the table: a creature card with an alternative cost.
pub struct AltRow {
    /// The card name.
    pub name: &'static str,
    /// Its printed mana cost.
    pub cost: &'static str,
    /// The alternative cost: the way it is remembered by.
    pub way: CastWay,
    /// The mana of the alternative cost.
    pub mana: &'static str,
}

/// Every card of the table.
pub const ROWS: [AltRow; 4] = [
    AltRow {
        name: "Surge Beast",
        cost: "{4}{R}{R}",
        way: CastWay::Surge,
        mana: "{R}",
    },
    AltRow {
        name: "Web Beast",
        cost: "{3}{W}",
        way: CastWay::WebSlinging,
        mana: "{W}",
    },
    AltRow {
        name: "Warp Beast",
        cost: "{3}{R}",
        way: CastWay::Warp,
        mana: "{R}",
    },
    AltRow {
        name: "Impending Beast",
        cost: "{4}{G}",
        way: CastWay::Impending,
        mana: "{1}{G}",
    },
];

fn mana(text: &str) -> ManaCost {
    ManaCost::parse(text).unwrap_or_default()
}

/// "You cast another spell this turn": the spells the caster cast, counted before the one being cast.
fn spell_cast_this_turn() -> Condition {
    Condition::Compare {
        left: Amount::Tallied {
            who: Ref::You,
            what: TurnTally::SpellsCast,
        },
        op: Cmp::Ge,
        right: Amount::Fixed(1),
    }
}

/// The cost of the row: its mana, and for web-slinging the return of a tapped creature.
fn alt_cost(row: &AltRow) -> Cost {
    let mut items = vec![CostItem::Mana(
        row.mana.parse::<ManaCostX>().unwrap_or_default(),
    )];
    if row.way == CastWay::WebSlinging {
        items.push(CostItem::ReturnToHand {
            filter: Selector(vec![
                Pred::Type(CardType::Creature),
                Pred::ControlledBy(Ref::You),
                Pred::Tapped,
            ]),
            count: Amount::one(),
        });
    }
    Cost(items)
}

fn alt_cast(row: &AltRow) -> Ability {
    Ability::AltCast {
        way: row.way,
        cost: Some(alt_cost(row)),
        from: ZoneKind::Hand,
        condition: (row.way == CastWay::Surge).then(spell_cast_this_turn),
        flash: false,
    }
}

/// Warp: "At the beginning of the next end step, exile it. You may cast it from exile on a later turn."
fn warp_riders() -> Vec<Ability> {
    let exile_then_cast = vec![
        Effect::Exile {
            what: Ref::Bound(WARPED.to_owned()),
            bind: Some(EXILED.to_owned()),
            duration: None,
        },
        Effect::Rule {
            rule: GameRule::MayCast(CastPermission {
                who: Ref::You,
                what: Selector::default(),
                from: Some(ZoneKind::Exile),
                free: false,
                flash: false,
                only: Some(Ref::Bound(EXILED.to_owned())),
                play: false,
                once: false,
                additional: None,
            }),
            duration: Duration::Forever,
        },
    ];
    vec![Ability::Triggered {
        when: TriggerPattern::ZoneChange {
            what: Selector::this(),
            from: None,
            to: Some(ZoneKind::Battlefield),
        },
        intervening_if: Some(Condition::CastWith(CastWay::Warp)),
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: vec![
            Effect::Choose {
                who: Ref::You,
                from: Ref::This,
                count: Bounds::default(),
                bind: WARPED.to_owned(),
                rest_as: None,
            },
            Effect::DelayedTrigger {
                when: TriggerPattern::StepBegins {
                    step: Step::End,
                    whose: Ref::EachPlayer,
                },
                once: true,
                this_turn: false,
                effects: exile_then_cast,
            },
        ],
    }]
}

fn time() -> CounterKind {
    CounterKind::Other("time".to_owned())
}

/// Impending 4: "enters with four time counters", "isn't a creature while it has a time counter".
fn impending_riders() -> Vec<Ability> {
    vec![
        Ability::Replacement {
            when: ReplaceWhen::Enters {
                what: Selector::this(),
            },
            by: ReplaceBy::WithCounters {
                counter: time(),
                count: Amount::Fixed(4),
                tapped: false,
            },
            condition: Some(Condition::CastWith(CastWay::Impending)),
            from: ZoneKind::Battlefield,
        },
        Ability::Static {
            affected: Ref::This,
            mods: vec![Modification::RemoveType(CardType::Creature)],
            condition: Some(Condition::Compare {
                left: Amount::Counters {
                    on: Ref::This,
                    counter: time(),
                },
                op: Cmp::Gt,
                right: Amount::Fixed(0),
            }),
            defining: false,
            from: ZoneKind::Battlefield,
        },
    ]
}

/// The card of a row: a 3/3 creature with its alternative cost and what the way does to the permanent.
pub fn card(row: &AltRow) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(row.name)
        .with_type(CardType::Creature)
        .with_mana_cost(mana(row.cost))
        .with_power_toughness(3, 3)
        .with_rules(alt_cast(row));
    let riders = match row.way {
        CastWay::Warp => warp_riders(),
        CastWay::Impending => impending_riders(),
        _ => Vec::new(),
    };
    for rider in riders {
        card = card.with_rules(rider);
    }
    Arc::new(card)
}

/// The row of the card called `name`.
pub fn row(name: &str) -> Option<&'static AltRow> {
    ROWS.iter().find(|row| row.name == name)
}

/// A legal Normal deck of lands of the colours of the table, three of each card of the table and cheap creatures to cast
/// first (surge) and to attack with, derived entirely from `rng`.
pub fn alt_cast_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Red, Color::White, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 3])).collect();
    for row in &ROWS {
        deck.extend(std::iter::repeat_n(card(row), 3));
    }
    let mut slot = 0;
    while deck.len() < 60 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Alt Cast Fodder {slot} {power}/{power}");
        deck.extend(std::iter::repeat_n(
            creature(&name, 1, &[colors[rng.index(3)]], power, power),
            2,
        ));
        slot += 1;
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`alt_cast_deck`]s, fully determined by `seed`.
pub fn alt_cast_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| alt_cast_deck(&mut GameRng::substream(seed, 37_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
