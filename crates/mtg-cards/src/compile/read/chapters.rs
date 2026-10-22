//! Saga chapters (CR 714): "I — Draw a card." and "II, III — Gain 2 life." are triggered abilities of the Saga
//! that trigger when its lore counters reach their number, so a line reads like a spell ability and becomes one chapter
//! ability for each number it names.

use super::{Own, clause_with};
use crate::ability::Ability;
use crate::compile::{ClauseKind, classify};
use crate::refs::ZoneKind;
use crate::trigger::TriggerPattern;

/// The chapter abilities of a chapter line (and the bullets under it): one for each number, each with the effects the
/// text reads into; `None` when the text is not read (the line stays with the table).
pub fn block(text: &str, x_free: bool, own: &Own) -> Option<Vec<Ability>> {
    let mut lines = text.lines();
    let (numbers, first) = classify::chapter(lines.next()?)?;
    let body = std::iter::once(first)
        .chain(lines)
        .collect::<Vec<_>>()
        .join("\n");
    let kind = if body.starts_with("Choose ") {
        ClauseKind::Modal
    } else {
        ClauseKind::Spell
    };
    let Ability::Spell { targets, effects } = clause_with(&body, kind, x_free, own)? else {
        return None;
    };
    Some(
        numbers
            .into_iter()
            .map(|number| Ability::Triggered {
                when: TriggerPattern::Chapter { number },
                intervening_if: None,
                limit: None,
                from: ZoneKind::Battlefield,
                targets: targets.clone(),
                effects: effects.clone(),
            })
            .collect(),
    )
}
