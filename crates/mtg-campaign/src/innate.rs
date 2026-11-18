//! Boss innates: each boss's one fixed effect, written once in content with a number N that grows over five levels
//! (`"levels": [n1, …, n5]`, the text's `{N}` and every `"$N"` in its abilities). Loading makes one card per level;
//! the fight takes the level [`Scale::innate_level`](crate::scale::Scale::innate_level) names.

use serde_json::Value;

use crate::CampaignError;

/// The levels an innate is written in, I to V.
pub const LEVELS: u8 = 5;

/// The roman numeral of `level` (1 to [`LEVELS`]).
pub fn numeral(level: u8) -> &'static str {
    match level {
        0 | 1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        _ => "V",
    }
}

/// The id of the card of innate `id` at `level`.
pub fn innate_id(id: &str, level: u8) -> String {
    format!("{id}_{}", numeral(level).to_lowercase())
}

/// The card files' entries with each innate written with `levels` replaced by its five level cards; other entries as
/// they are.
pub(crate) fn expand(entries: Vec<Value>) -> Result<Vec<Value>, CampaignError> {
    let mut out = Vec::with_capacity(entries.len());
    for mut entry in entries {
        let Some(levels) = entry.as_object_mut().and_then(|e| e.remove("levels")) else {
            out.push(entry);
            continue;
        };
        let numbers: Vec<u64> = serde_json::from_value(levels)
            .map_err(|e| CampaignError::Content(format!("innate levels: {e}")))?;
        if numbers.len() != usize::from(LEVELS) || numbers.windows(2).any(|w| w[1] < w[0]) {
            return Err(CampaignError::Content(format!(
                "innate {} needs {LEVELS} levels that never fall",
                entry["id"]
            )));
        }
        for (at, n) in (1..=LEVELS).zip(numbers) {
            out.push(at_level(&entry, at, n));
        }
    }
    Ok(out)
}

/// `entry` at `level`, its number `n`: id, name, level, text and abilities.
fn at_level(entry: &Value, level: u8, n: u64) -> Value {
    let mut card = entry.clone();
    set_n(&mut card["abilities"], n);
    let id = card["id"].as_str().unwrap_or_default().to_owned();
    let name = card["name"].as_str().unwrap_or_default().to_owned();
    let text = card["text"]
        .as_str()
        .unwrap_or_default()
        .replace("{N}", &n.to_string());
    card["id"] = Value::from(innate_id(&id, level));
    card["name"] = Value::from(format!("{name} {}", numeral(level)));
    card["text"] = Value::from(text);
    card["level"] = Value::from(level);
    card
}

/// Puts `n` wherever `value` holds the string `"$N"`.
fn set_n(value: &mut Value, n: u64) {
    match value {
        Value::String(s) if s == "$N" => *value = Value::from(n),
        Value::Array(items) => items.iter_mut().for_each(|v| set_n(v, n)),
        Value::Object(map) => map.values_mut().for_each(|v| set_n(v, n)),
        _ => {}
    }
}
