//! Boss innates: each boss's one fixed effect, written once in content with a number N that grows over five levels
//! (`"levels": [n1, …, n5]`, the text's `{N}` and every `"$N"` in its abilities). A level may differ in more than N:
//! `"params": {"NAME": [v1, …, v5]}` puts any JSON value wherever `"$NAME"` stands in the abilities (an ability that
//! becomes `null` is left out at that level), and
//! `"texts": [t1, …, t5]` gives each level its own words (`{N}` still filled in). Loading makes one card per level;
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
        let params = take_per_level(&mut entry, "params")?;
        let texts = take_per_level(&mut entry, "texts")?;
        let numbers: Vec<u64> = serde_json::from_value(levels)
            .map_err(|e| CampaignError::Content(format!("innate levels: {e}")))?;
        if numbers.len() != usize::from(LEVELS) || numbers.windows(2).any(|w| w[1] < w[0]) {
            return Err(CampaignError::Content(format!(
                "innate {} needs {LEVELS} levels that never fall",
                entry["id"]
            )));
        }
        for (at, n) in (1..=LEVELS).zip(numbers) {
            let mut card = at_level(&entry, at, n, texts.as_ref());
            if let Some(Value::Object(params)) = &params {
                for (name, values) in params {
                    let value = values[usize::from(at - 1)].clone();
                    set(&mut card["abilities"], &format!("${name}"), &value);
                }
            }
            if let Some(abilities) = card["abilities"].as_array_mut() {
                abilities.retain(|a| !a.is_null());
            }
            out.push(card);
        }
    }
    Ok(out)
}

/// Takes `key` off an innate entry: a list of [`LEVELS`] values, or an object of such lists; none when absent.
fn take_per_level(entry: &mut Value, key: &str) -> Result<Option<Value>, CampaignError> {
    let Some(value) = entry.as_object_mut().and_then(|e| e.remove(key)) else {
        return Ok(None);
    };
    let lists: Vec<&Value> = match &value {
        Value::Object(map) => map.values().collect(),
        other => vec![other],
    };
    let five = |list: &&Value| {
        list.as_array()
            .is_some_and(|l| l.len() == usize::from(LEVELS))
    };
    if !lists.iter().all(five) {
        return Err(CampaignError::Content(format!(
            "innate {} {key} needs {LEVELS} values each",
            entry["id"]
        )));
    }
    Ok(Some(value))
}

/// `entry` at `level`, its number `n`: id, name, level, text (the level's own words when `texts` gives them) and
/// abilities.
fn at_level(entry: &Value, level: u8, n: u64, texts: Option<&Value>) -> Value {
    let mut card = entry.clone();
    set(&mut card["abilities"], "$N", &Value::from(n));
    let id = card["id"].as_str().unwrap_or_default().to_owned();
    let name = card["name"].as_str().unwrap_or_default().to_owned();
    let text = texts
        .map_or(&card["text"], |t| &t[usize::from(level - 1)])
        .as_str()
        .unwrap_or_default()
        .replace("{N}", &n.to_string());
    card["id"] = Value::from(innate_id(&id, level));
    card["name"] = Value::from(format!("{name} {}", numeral(level)));
    card["text"] = Value::from(text);
    card["level"] = Value::from(level);
    card
}

/// Puts `to` wherever `value` holds the string `name`.
fn set(value: &mut Value, name: &str, to: &Value) {
    match value {
        Value::String(s) if s == name => *value = to.clone(),
        Value::Array(items) => items.iter_mut().for_each(|v| set(v, name, to)),
        Value::Object(map) => map.values_mut().for_each(|v| set(v, name, to)),
        _ => {}
    }
}
