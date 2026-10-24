//! Station blocks (CR 721): "9+ | Flying, first strike" and the lines under it are abilities of the Spacecraft that work
//! while it has that many charge counters or more, so each is read like any other line of text and conditioned on the
//! number of charge counters. The block of the highest threshold also makes a Spacecraft with a printed power and
//! toughness an artifact creature.

use super::Own;
use super::levels::Plan;
use crate::ability::Ability;
use crate::compile::clauses;
use crate::modify::Modification;
use crate::refs::{Ref, ZoneKind};
use crate::value::{Amount, Cmp, Condition, CounterKind};
use mtg_core::card::CardType;

/// The abilities of a station block: the creature type when the threshold is where the Spacecraft becomes a creature
/// (`Own::creature_at`), then every line of the block, each conditioned on the charge counters; `None` when any line is
/// not read (the whole block stays with the table).
pub fn block(text: &str, x_free: bool, own: &Own) -> Option<Vec<Ability>> {
    plan(text, own.creature_at)?.read(x_free, own)
}

/// The plan of a station block: the creature type where the Spacecraft becomes one, and the lines (the text after the
/// bar of the header, then the rest) under the charge counters.
pub(super) fn plan(text: &str, creature_at: Option<i32>) -> Option<Plan> {
    let mut lines = text.lines();
    let (threshold, first) = clauses::station_header(lines.next()?)?;
    let charged = Condition::Compare {
        left: Amount::Counters {
            on: Ref::This,
            counter: CounterKind::Charge,
        },
        op: Cmp::Ge,
        right: Amount::Fixed(threshold),
    };
    let mut head = Vec::new();
    if creature_at == Some(threshold) {
        let creature = Ability::Static {
            affected: Ref::This,
            mods: vec![Modification::AddType(CardType::Creature)],
            condition: Some(charged.clone()),
            defining: false,
            from: ZoneKind::Battlefield,
        };
        head.push((format!("artifact creature at {threshold}+"), creature));
    }
    Some(Plan {
        condition: charged,
        head,
        lines: std::iter::once(first)
            .chain(lines)
            .map(str::to_owned)
            .collect(),
    })
}
