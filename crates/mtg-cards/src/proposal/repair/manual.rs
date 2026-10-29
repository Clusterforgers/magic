//! A `Manual` ability only says that a clause is left to the table, and the checker leaves every clause no ability
//! stands for to the table anyway. A `Manual` whose span cannot be used is therefore dropped, not guessed at.

use serde_json::Value;

use super::{Repair, RepairClass, body_of};

/// The span of a `Manual` as the file gives it.
fn span_of(ability: &mut Value) -> Option<(u64, u64)> {
    let (name, body) = body_of(ability)?;
    if name != "Manual" {
        return None;
    }
    let span = body.get("span")?.as_object()?;
    Some((span.get("start")?.as_u64()?, span.get("end")?.as_u64()?))
}

/// Drops each `Manual` whose span is empty, backwards, past the end of the text or overlapping an earlier `Manual`.
pub(super) fn repair(abilities: &mut Vec<Value>, text: &str, at: &str, repairs: &mut Vec<Repair>) {
    let length = u64::try_from(text.chars().count()).unwrap_or(u64::MAX);
    let mut kept: Vec<(u64, u64)> = Vec::new();
    let mut index = 0;
    abilities.retain_mut(|ability| {
        let n = index;
        index += 1;
        let Some((start, end)) = span_of(ability) else {
            return true;
        };
        let why = if start >= end {
            "is empty or backwards"
        } else if end > length {
            "ends past the oracle text"
        } else if kept.iter().any(|(s, e)| start < *e && *s < end) {
            "overlaps an earlier Manual"
        } else {
            kept.push((start, end));
            return true;
        };
        repairs.push(Repair {
            class: RepairClass::ManualSpan,
            at: format!("{at}.abilities[{n}].Manual"),
            note: format!("the span {start}..{end} {why}: dropped, its clause stays with the table unless an ability stands for it"),
        });
        false
    });
}
