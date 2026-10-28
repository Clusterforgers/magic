//! The modes of a spell that chooses a fixed number of them, each written as a spell of its own with every wording the
//! checker accepts: "Choose three. You may choose the same mode more than once. • ... • ... • ...". The renderers
//! write each mode with their own wordings only.

use serde_json::Value;

use super::c6_mr_written;
use crate::compile::read::amounts::count_word;

/// The most wordings one ability gets: the modes multiply their wordings.
const C6_MR_MOST: usize = 512;

/// Adds the wordings of a spell whose only step is a choice of modes.
pub(super) fn add(json: &str, texts: &mut Vec<String>) {
    let Ok(value) = serde_json::from_str::<Value>(json) else {
        return;
    };
    let Some(spell) = value
        .get("Spell")
        .filter(|spell| spell.get("targets").is_none())
    else {
        return;
    };
    let Some([step]) = spell
        .get("effects")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    else {
        return;
    };
    let Some(modal) = step.get("Modal") else {
        return;
    };
    let Some(head) = c6_mr_head(modal) else {
        return;
    };
    let Some(modes) = modal.get("modes").and_then(Value::as_array) else {
        return;
    };
    let mut written = vec![head];
    for mode in modes {
        let mut alone = serde_json::Map::new();
        for key in ["targets", "effects"] {
            if let Some(part) = mode.get(key) {
                alone.insert(key.to_owned(), part.clone());
            }
        }
        let alone = serde_json::json!({ "Spell": alone });
        let lines = c6_mr_written(&alone.to_string());
        if lines.is_empty() {
            return;
        }
        let mut next = Vec::new();
        'all: for before in &written {
            for line in &lines {
                if next.len() >= C6_MR_MOST {
                    break 'all;
                }
                next.push(format!("{before}\n\u{2022} {line}"));
            }
        }
        written = next;
    }
    texts.extend(written);
}

/// "Choose three. You may choose the same mode more than once." or "Choose two —" for a choice of a fixed number of
/// modes that is not at random and not limited to once each turn.
fn c6_mr_head(modal: &Value) -> Option<String> {
    let bounds = modal.get("choose")?;
    let min = bounds.get("min")?.as_u64()?;
    if bounds.get("max")?.as_u64()? != min || min < 2 {
        return None;
    }
    for key in ["at_random", "once_each_turn", "all_if"] {
        if modal.get(key).is_some() {
            return None;
        }
    }
    let word = count_word(u32::try_from(min).ok()?)?;
    let repeat = modal.get("repeat") == Some(&Value::Bool(true));
    Some(if repeat {
        format!("Choose {word}. You may choose the same mode more than once.")
    } else {
        format!("Choose {word} \u{2014}")
    })
}
