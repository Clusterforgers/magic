//! Day and night (CR 726): "It becomes night.", "If it's night, it becomes day. Otherwise, it becomes night.",
//! "it's day" as a condition, "Whenever day becomes night or night becomes day" and "If it's neither day nor night,
//! it becomes day as ~ enters." (`Effect::BecomeDayNight`, `Condition::IsDay`, `IsNight` and `NeitherDayNorNight`,
//! `TriggerPattern::DayNightChanges`, `ReplaceBy::DayIfNeither`). The writer's side is the `*_text` functions.

use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen, TriggerPattern};
use crate::value::{Condition, DayNight};

/// The word for the designation.
const fn word(to: DayNight) -> &'static str {
    match to {
        DayNight::Day => "day",
        DayNight::Night => "night",
    }
}

/// "day" or "night".
fn designation(r: &mut Reader<'_>) -> Option<DayNight> {
    if r.word("day") {
        Some(DayNight::Day)
    } else {
        r.word("night").then_some(DayNight::Night)
    }
}

/// "it becomes night": the effect of one clause.
fn becomes_clause(r: &mut Reader<'_>) -> Option<Effect> {
    r.need("it becomes")?;
    Some(Effect::BecomeDayNight {
        to: designation(r)?,
    })
}

/// "It becomes night.": the game becomes day or night.
pub fn becomes(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    Some(vec![becomes_clause(r)?])
}

/// "If it's night, it becomes day. Otherwise, it becomes night.": one of the two, whichever the game is not.
pub fn toggle(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("if")?;
    let condition = condition(r).filter(|held| *held != Condition::NeitherDayNorNight)?;
    r.punct(',').then_some(())?;
    let then = becomes_clause(r)?;
    r.punct('.').then_some(())?;
    r.need("otherwise")?;
    r.punct(',').then_some(())?;
    let otherwise = becomes_clause(r)?;
    Some(vec![Effect::If {
        condition,
        then: vec![then],
        otherwise: vec![otherwise],
    }])
}

/// The effects `becomes` and `toggle` read, written back.
pub fn effect_text(effect: &Effect) -> Option<String> {
    match effect {
        Effect::BecomeDayNight { to } => Some(format!("it becomes {}", word(*to))),
        Effect::If {
            condition,
            then,
            otherwise,
        } => {
            let ([Effect::BecomeDayNight { to: first }], [Effect::BecomeDayNight { to: second }]) =
                (then.as_slice(), otherwise.as_slice())
            else {
                return None;
            };
            Some(format!(
                "if {}, it becomes {}. Otherwise, it becomes {}",
                condition_text(condition)?,
                word(*first),
                word(*second)
            ))
        }
        _ => None,
    }
}

/// "it's day", "it's night", "it's neither day nor night": how the designation stands.
pub fn condition(r: &mut Reader<'_>) -> Option<Condition> {
    // Callers try it among other conditions without a rollback, so a miss leaves the reader where it was.
    r.attempt(|r| {
        r.need("it's")?;
        if r.words("neither day nor night") {
            return Some(Condition::NeitherDayNorNight);
        }
        Some(match designation(r)? {
            DayNight::Day => Condition::IsDay,
            DayNight::Night => Condition::IsNight,
        })
    })
}

/// The condition `condition` reads, written back.
pub fn condition_text(condition: &Condition) -> Option<String> {
    Some(match condition {
        Condition::IsDay => "it's day".to_owned(),
        Condition::IsNight => "it's night".to_owned(),
        Condition::NeitherDayNorNight => "it's neither day nor night".to_owned(),
        _ => return None,
    })
}

/// "day becomes night or night becomes day", "night becomes day", "day becomes night": the event of a trigger, after
/// "whenever".
pub fn changes(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    if r.words("day becomes night or night becomes day") {
        return Some(TriggerPattern::DayNightChanges { to: None });
    }
    let to = if r.words("night becomes day") {
        DayNight::Day
    } else {
        r.need("day becomes night")?;
        DayNight::Night
    };
    Some(TriggerPattern::DayNightChanges { to: Some(to) })
}

/// The event `changes` reads, written back after "Whenever".
pub const fn changes_text(to: Option<DayNight>) -> &'static str {
    match to {
        None => "day becomes night or night becomes day",
        Some(DayNight::Day) => "night becomes day",
        Some(DayNight::Night) => "day becomes night",
    }
}

/// "If it's neither day nor night, it becomes day as ~ enters.": the card starts the day as it enters.
pub fn starts_the_day(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("if")?;
    (condition(r)? == Condition::NeitherDayNorNight).then_some(())?;
    r.punct(',').then_some(())?;
    r.need("it becomes day as")?;
    r.source().then_some(())?;
    r.need("enters")?;
    Some(Ability::Replacement {
        when: ReplaceWhen::Enters {
            what: Selector::this(),
        },
        by: ReplaceBy::DayIfNeither,
        condition: None,
        from: ZoneKind::Battlefield,
    })
}
