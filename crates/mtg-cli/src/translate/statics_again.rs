//! `Mode$ Panharmonicon`: a triggered ability of the permanents `ValidCard$` names triggers an additional time when the
//! event `ValidMode$` names, caused by `ValidCause$`, makes it trigger (`Rule.TriggerAgain`).

use std::fmt::Write;

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The `Rule.TriggerAgain` of a `Panharmonicon` static line.
pub fn again(ctx: &Ctx, p: &Params) -> Tr<Value> {
    super::ctx::check_keys(
        p,
        "Static.Panharmonicon",
        &[
            "ValidMode",
            "ValidCard",
            "ValidCause",
            "Origin",
            "Destination",
            "ValidActivator",
            "ValidPlayer",
        ],
        |key, value| key == "Mode" || (key == "EffectZone" && value == "Battlefield"),
    )?;
    let Some(modes) = p.get("ValidMode") else {
        // No event named: every trigger of the permanents repeats; a cause parameter without a mode stays a gap.
        if [
            "ValidCause",
            "Origin",
            "Destination",
            "ValidActivator",
            "ValidPlayer",
        ]
        .iter()
        .any(|key| p.has(key))
        {
            return unmapped("static:Panharmonicon-any-cause");
        }
        return Ok(json!({"TriggerAgain": again_source(ctx, p, None)?}));
    };
    let mut causes: Vec<Value> = Vec::new();
    for mode in modes.split(',') {
        for cause in cause(ctx, p, mode)? {
            // "Cast or copy" is two events; a list of events is one list.
            let parts = match cause.get("AnyOf").and_then(Value::as_array) {
                Some(parts) => parts.clone(),
                None => vec![cause],
            };
            for part in parts {
                if !causes.contains(&part) {
                    causes.push(part);
                }
            }
        }
    }
    let cause = match causes.len() {
        0 => return unmapped("static:Panharmonicon-no-mode"),
        1 => causes.remove(0),
        _ => json!({ "AnyOf": causes }),
    };
    Ok(json!({"TriggerAgain": again_source(ctx, p, Some(cause))?}))
}

/// The body of a `TriggerAgain` rule: the event that causes it (any event when absent) and the permanents it covers.
fn again_source(ctx: &Ctx, p: &Params, cause: Option<Value>) -> Tr<Value> {
    let mut rule = json!({});
    if let Some(cause) = cause {
        rule["cause"] = cause;
    }
    match p.get("ValidCard").unwrap_or("Permanent.YouCtrl") {
        // Only the controller's permanents trigger again, so the plain word needs no filter.
        "Permanent.YouCtrl" => {}
        valid => rule["source"] = json!(selector_any(ctx, valid)?),
    }
    Ok(rule)
}

/// The event patterns one `ValidMode$` word names; the batch and one-target forms of an event are the event itself.
fn cause(ctx: &Ctx, p: &Params, mode: &str) -> Tr<Vec<Value>> {
    let line = |text: String| Params::parse(&text);
    let from = |key: &str| p.get(key).map(str::to_owned);
    let valid = from("ValidCause");
    let pattern = |_: &str, params: Params| super::triggers::pattern(ctx, &params);
    match mode {
        "ChangesZone" | "ChangesZoneAll" => {
            let mut text = format!(
                "Mode$ ChangesZone | ValidCard$ {}",
                valid.as_deref().unwrap_or("Card")
            );
            for key in ["Origin", "Destination"] {
                if let Some(zone) = from(key) {
                    let _ = write!(text, " | {key}$ {zone}");
                }
            }
            Ok(vec![pattern(mode, line(text))?])
        }
        "Attacks" | "AttackersDeclared" | "AttackersDeclaredOneTarget" => {
            let text = format!(
                "Mode$ Attacks | ValidCard$ {}",
                valid.as_deref().unwrap_or("Creature")
            );
            Ok(vec![pattern(mode, line(text))?])
        }
        "SpellCast" | "SpellCopy" | "SpellCastOrCopy" => {
            let mut text = "Mode$ SpellCastOrCopy".to_owned();
            if let Some(valid) = &valid {
                let _ = write!(text, " | ValidCard$ {valid}");
            }
            if let Some(who) = from("ValidActivator") {
                let _ = write!(text, " | ValidActivatingPlayer$ {who}");
            }
            let both = pattern(mode, line(text))?;
            let wanted = match mode {
                "SpellCast" => "SpellCast",
                "SpellCopy" => "SpellCopied",
                _ => return Ok(vec![both]),
            };
            Ok(both["AnyOf"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|part| part.get(wanted).is_some())
                .cloned()
                .collect())
        }
        "Drawn" => Ok(vec![pattern(mode, line("Mode$ Drawn".to_owned()))?]),
        "LifeGained" => {
            let text = format!(
                "Mode$ LifeGained | ValidPlayer$ {}",
                from("ValidPlayer").as_deref().unwrap_or("Player")
            );
            Ok(vec![pattern(mode, line(text))?])
        }
        other => unmapped(format!("static:Panharmonicon-mode={other}")),
    }
}
