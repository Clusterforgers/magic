//! Level blocks (CR 711): "LEVEL 2-3", the power and toughness line and the abilities under it are abilities of the
//! creature that work while its level counters are in range, so each is read like any other line of text and
//! conditioned on the number of level counters.

use super::{Own, clause_with};
use crate::ability::Ability;
use crate::compile::{ClauseKind, classify, keywords};
use crate::modify::Modification;
use crate::refs::{Ref, ZoneKind};
use crate::value::{Amount, Cmp, Condition, CounterKind};

/// The abilities of a level block: its power and toughness and every line under them, each conditioned on the level
/// counters in the header's range; `None` when any line is not read (the whole block stays with the table).
pub fn block(text: &str, x_free: bool, own: &Own) -> Option<Vec<Ability>> {
    let mut lines = text.lines();
    let range = header(lines.next()?)?;
    let (power, toughness) = size(lines.next()?)?;
    let mut abilities = vec![Ability::Static {
        affected: Ref::This,
        mods: vec![Modification::SetPT {
            power: Amount::Fixed(power),
            toughness: Amount::Fixed(toughness),
        }],
        condition: Some(range.clone()),
        defining: false,
        from: ZoneKind::Battlefield,
    }];
    for line in lines {
        abilities.push(under_the_header(line, &range, x_free, own)?);
    }
    Some(abilities)
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
    let (Ability::Static { condition, .. }
    | Ability::Rule { condition, .. }
    | Ability::Replacement { condition, .. }
    | Ability::CostModifier { condition, .. }
    | Ability::Activated { condition, .. }
    | Ability::Triggered {
        intervening_if: condition,
        ..
    }) = &mut ability
    else {
        return None;
    };
    *condition = Some(match condition.take() {
        Some(own) => Condition::All(vec![level.clone(), own]),
        None => level.clone(),
    });
    Some(ability)
}

/// The keywords of a line, which the creature has under the header ("Flying", "First strike, lifelink").
fn given(line: &str) -> Option<Ability> {
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
