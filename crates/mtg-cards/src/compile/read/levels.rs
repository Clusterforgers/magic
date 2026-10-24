//! Level blocks (CR 711): "LEVEL 2-3", the power and toughness line and the abilities under it are abilities of the
//! creature that work while its level counters are in range, so each is read like any other line of text and
//! conditioned on the number of level counters. A `Plan` splits a level, Class or station block into the abilities of
//! its header and the lines under it, for the readers and for the checker of proposals.

use super::{Own, clause_with};
use crate::ability::Ability;
use crate::compile::{ClauseKind, classify, clauses, keywords};
use crate::modify::Modification;
use crate::refs::{Ref, ZoneKind};
use crate::value::{Amount, Cmp, Condition, CounterKind};

/// A block of the text split into what it gives: the condition its lines work under, the abilities its header makes and
/// the lines under the header.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// What the lines work while: the level counters in range, the level of a Class or the charge counters.
    pub condition: Condition,
    /// The abilities of the header and of its fixed line, each after the text it stands for.
    pub head: Vec<(String, Ability)>,
    /// The lines under the header: each is one ability that works while the condition holds.
    pub lines: Vec<String>,
}

impl Plan {
    /// The abilities of the block as the readers make them: the head, then each line under the condition; `None` when a
    /// line is not read.
    pub fn read(&self, x_free: bool, own: &Own) -> Option<Vec<Ability>> {
        let mut abilities: Vec<Ability> = self.head.iter().map(|(_, a)| a.clone()).collect();
        for line in &self.lines {
            abilities.push(under_the_header(line, &self.condition, x_free, own)?);
        }
        Some(abilities)
    }
}

/// The plan of a level, Class or station block; `creature_at` is where a Spacecraft with a printed power becomes a
/// creature.
pub fn plan(text: &str, creature_at: Option<i32>) -> Option<Plan> {
    let head = text.lines().next()?;
    if head.starts_with("LEVEL ") {
        level_plan(text)
    } else if clauses::class_header(head).is_some() {
        super::class::plan(text)
    } else {
        super::stations::plan(text, creature_at)
    }
}

/// The abilities of a level block: its power and toughness and every line under them, each conditioned on the level
/// counters in the header's range; `None` when any line is not read (the whole block stays with the table).
pub fn block(text: &str, x_free: bool, own: &Own) -> Option<Vec<Ability>> {
    level_plan(text)?.read(x_free, own)
}

/// "LEVEL 2-3", "2/4" and the lines under them.
fn level_plan(text: &str) -> Option<Plan> {
    let mut lines = text.lines();
    let range = header(lines.next()?)?;
    let sized = lines.next()?;
    let (power, toughness) = size(sized)?;
    let set = Ability::Static {
        affected: Ref::This,
        mods: vec![Modification::SetPT {
            power: Amount::Fixed(power),
            toughness: Amount::Fixed(toughness),
        }],
        condition: Some(range.clone()),
        defining: false,
        from: ZoneKind::Battlefield,
    };
    Some(Plan {
        condition: range,
        head: vec![(sized.to_owned(), set)],
        lines: lines.map(str::to_owned).collect(),
    })
}

/// "LEVEL 2-3" (between two and three level counters) and "LEVEL 4+" (four or more), as the condition they name.
fn header(line: &str) -> Option<Condition> {
    let range = line.strip_prefix("LEVEL ")?;
    if let Some(least) = range.strip_suffix('+') {
        return Some(levels(Cmp::Ge, least.parse().ok()?));
    }
    let (least, most) = range.split_once('-')?;
    let (least, most): (i32, i32) = (least.parse().ok()?, most.parse().ok()?);
    (least <= most).then(|| Condition::All(vec![levels(Cmp::Ge, least), levels(Cmp::Le, most)]))
}

/// The level counters on the creature compare to the number this way.
fn levels(op: Cmp, number: i32) -> Condition {
    Condition::Compare {
        left: Amount::Counters {
            on: Ref::This,
            counter: CounterKind::Other("level".into()),
        },
        op,
        right: Amount::Fixed(number),
    }
}

/// "2/4": the power and toughness a level gives.
fn size(line: &str) -> Option<(i32, i32)> {
    let (power, toughness) = line.split_once('/')?;
    Some((power.parse().ok()?, toughness.parse().ok()?))
}

/// One line under a header as an ability that works only while the header's condition holds. A line of keywords gives
/// them to the creature; any other line is read as it would be anywhere else.
pub(super) fn under_the_header(
    line: &str,
    level: &Condition,
    x_free: bool,
    own: &Own,
) -> Option<Ability> {
    let kind = classify::classify(line, false);
    let mut ability = match kind {
        ClauseKind::Keyword => given(line)?,
        ClauseKind::Triggered | ClauseKind::Activated | ClauseKind::Static => {
            clause_with(line, kind, x_free, own)?
        }
        _ => return None,
    };
    let slot = condition_of(&mut ability)?;
    *slot = Some(match slot.take() {
        Some(own) => Condition::All(vec![level.clone(), own]),
        None => level.clone(),
    });
    Some(ability)
}

/// The condition an ability works under: the one thing a block adds to the ability a line reads as.
fn condition_of(ability: &mut Ability) -> Option<&mut Option<Condition>> {
    match ability {
        Ability::Static { condition, .. }
        | Ability::Rule { condition, .. }
        | Ability::Replacement { condition, .. }
        | Ability::CostModifier { condition, .. }
        | Ability::Activated { condition, .. }
        | Ability::Triggered {
            intervening_if: condition,
            ..
        } => Some(condition),
        _ => None,
    }
}

/// The ability of a line under a header without the header's condition: the inverse of [`under_the_header`]. The
/// condition is the header's alone, or one of the conditions of an `All`, in the order the reader makes or any other;
/// `None` when the ability does not work under it.
pub fn without_header(ability: &Ability, level: &Condition) -> Option<Ability> {
    let mut bare = ability.clone();
    let slot = condition_of(&mut bare)?;
    let mut rest = rest_of(slot.take()?, level)?;
    *slot = match rest.len() {
        0 => None,
        1 => rest.pop(),
        _ => Some(Condition::All(rest)),
    };
    Some(bare)
}

/// The conditions of `held` that are not the level; `None` when the level is not among them.
fn rest_of(held: Condition, level: &Condition) -> Option<Vec<Condition>> {
    if &held == level {
        return Some(Vec::new());
    }
    let Condition::All(mut parts) = held else {
        return None;
    };
    let wanted: Vec<&Condition> = match level {
        Condition::All(own) if !parts.contains(level) => own.iter().collect(),
        other => vec![other],
    };
    for part in wanted {
        let at = parts.iter().position(|held| held == part)?;
        parts.remove(at);
    }
    Some(parts)
}

/// The keywords of a line, which the creature has under the header ("Flying", "First strike, lifelink").
pub fn given(line: &str) -> Option<Ability> {
    let chars: Vec<char> = line.chars().collect();
    let mods = keywords::split_items(line)?
        .into_iter()
        .map(|range| {
            let item: String = chars[range].iter().collect();
            keywords::read(&item).map(Modification::AddKeyword)
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Ability::Static {
        affected: Ref::This,
        mods,
        condition: None,
        defining: false,
        from: ZoneKind::Battlefield,
    })
}
