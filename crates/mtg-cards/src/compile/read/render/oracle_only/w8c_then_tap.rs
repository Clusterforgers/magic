//! A tap or untap of the target the earlier effects named, said with a pronoun: "Put a +1/+1 counter on target
//! creature and untap it.", "Target creature gets +1/+3 and gains reach until end of turn. Untap it."

use serde_json::Value;

use super::w4d_parts::{parts, with_effects};
use crate::ability::Ability;

/// Adds the wordings of an ability whose last effect taps or untaps the target of the effects before it.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((value, effects)) = parts(ability) else {
        return;
    };
    let Some((last, before)) = effects.split_last() else {
        return;
    };
    let Some((verb, what)) = w8c_tap_verb(last) else {
        return;
    };
    if before.is_empty() || !before.iter().all(|effect| w8c_names(effect, what)) {
        return;
    }
    for lead in with_effects(&value, before) {
        let Some(body) = lead.strip_suffix('.') else {
            continue;
        };
        texts.push(format!("{body} and {verb} it."));
        texts.push(format!("{body}. {} it.", w8c_capital(verb)));
    }
}

/// The verb and the object of a tap or an untap of a target.
fn w8c_tap_verb(effect: &Value) -> Option<(&'static str, &Value)> {
    let (verb, body) = if let Some(body) = effect.get("Untap") {
        ("untap", body)
    } else {
        ("tap", effect.get("Tap")?)
    };
    let what = body.get("what")?;
    what.get("Target")?;
    Some((verb, what))
}

/// Whether the effect names the object somewhere: "it" then stands for it.
pub(super) fn w8c_names(effect: &Value, what: &Value) -> bool {
    effect == what
        || match effect {
            Value::Object(map) => map.values().any(|inner| w8c_names(inner, what)),
            Value::Array(items) => items.iter().any(|inner| w8c_names(inner, what)),
            _ => false,
        }
}

/// The verb with a capital, opening a sentence.
fn w8c_capital(verb: &str) -> &'static str {
    if verb == "untap" { "Untap" } else { "Tap" }
}
