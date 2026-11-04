//! What a damage replacement of a permanent does: Forge's `ReplaceWith$` part of an `Event$ DamageDone` line as the
//! DSL's `ReplaceBy` (prevent up to an amount, prevent and remove a counter, prevent and do something, do something
//! else instead, redirect the damage).

use serde_json::{Value, json};

use super::amount::counter_kind;
use super::ctx::{Ctx, Tr, check_keys, unmapped};
use crate::forge::script::Params;

/// The amount of an effect that reads the damage: the number of damage in the event, or a plain number.
fn damage_amount(ctx: &Ctx, word: Option<&str>) -> Tr<Value> {
    let Some(word) = word else {
        return Ok(json!(1));
    };
    if let Ok(number) = word.parse::<u32>() {
        return Ok(json!(number));
    }
    let expression = ctx.svar(word).unwrap_or_default();
    if expression == "ReplaceCount$DamageAmount" {
        return Ok(json!("Triggering"));
    }
    unmapped("replacement:DamageDone-amount")
}

/// The object a part's `Defined$` names: the recipient of the damage, the card itself, or what it is attached to.
fn defined(word: Option<&str>, shield: bool) -> Tr<Value> {
    match word {
        Some("ReplacedTarget") => Ok(json!("Triggering")),
        // In a shield "Self" would be the Forge `Effect` object, not the card whose ability made it.
        Some("Self") if !shield => Ok(json!("This")),
        Some("Equipped" | "Enchanted") if !shield => Ok(json!("Attached")),
        _ => unmapped("replacement:DamageDone-Defined"),
    }
}

/// Refuses a key the part does not read.
fn keys(part: &Params, read: &[&str]) -> Tr<()> {
    let label = format!(
        "Replacement.DamageDone.{}",
        part.get("DB").unwrap_or_default()
    );
    check_keys(part, &label, read, |key, _| key == "DB")
}

/// The effect a part runs with the damage in the event.
fn effect(ctx: &Ctx, part: &Params, recipient_is_you: bool, shield: bool) -> Tr<Value> {
    match part.get("DB").unwrap_or_default() {
        "PutCounter" => {
            keys(part, &["Defined", "CounterType", "CounterNum"])?;
            let Some(counter) = part.get("CounterType").and_then(counter_kind) else {
                return unmapped("replacement:DamageDone-counter");
            };
            Ok(json!({"PutCounters": {
                "on": defined(part.get("Defined"), shield)?,
                "counter": counter,
                "count": damage_amount(ctx, part.get("CounterNum"))?,
            }}))
        }
        "RemoveCounter" => {
            keys(part, &["Defined", "CounterType", "CounterNum"])?;
            let Some(counter) = part.get("CounterType").and_then(counter_kind) else {
                return unmapped("replacement:DamageDone-counter");
            };
            Ok(json!({"RemoveCounters": {
                "from": defined(part.get("Defined"), shield)?,
                "counter": counter,
                "count": damage_amount(ctx, part.get("CounterNum"))?,
            }}))
        }
        "GainLife" if recipient_is_you => {
            keys(part, &["Defined", "LifeAmount"])?;
            if part.get("Defined") != Some("ReplacedTarget") {
                return unmapped("replacement:DamageDone-Defined");
            }
            Ok(json!({"GainLife": {"amount": damage_amount(ctx, part.get("LifeAmount"))?}}))
        }
        other => unmapped(format!("replacement:DamageDone-{other}")),
    }
}

/// The `ReplaceBy` of a damage replacement that names a `ReplaceWith$` part. `recipient_is_you` is whether the line
/// is about damage to its controller (the only player a life gain may name); a `shield` is made by an `Effect` object.
pub fn by(ctx: &Ctx, p: &Params, name: &str, recipient_is_you: bool, shield: bool) -> Tr<Value> {
    let Some(text) = ctx.svar(name) else {
        return unmapped("svar:missing");
    };
    let part = Params::parse(text);
    if p.has("DamageTarget") && part.get("VarName") != Some("Affected") {
        return unmapped("key:Replacement.DamageDone.DamageTarget");
    }
    let prevents = p.get("PreventionEffect") == Some("True");
    match (part.get("DB").unwrap_or_default(), prevents) {
        // The effect object exiles itself: the shield serves one damage event.
        ("ChangeZone", true) if shield && exiles_the_effect(&part) => {
            keys(&part, &["Defined", "Origin", "Destination"])?;
            Ok(json!("PreventNext"))
        }
        ("ReplaceDamage", true) => {
            keys(&part, &["Amount"])?;
            // Half the damage, rounded up: only a static line prevents it (a shield keeps a fixed total).
            let half = part.get("Amount").and_then(|var| ctx.svar(var));
            if !shield && half == Some("ReplaceCount$DamageAmount/HalfUp") {
                return Ok(json!("PreventHalf"));
            }
            match part.get("Amount").map(str::parse::<u32>) {
                Some(Ok(most)) if most > 0 => Ok(json!({"PreventUpTo": most})),
                _ => unmapped("replacement:DamageDone-prevented-amount"),
            }
        }
        ("RemoveCounter", true) if part.get("Defined") == Some("Self") && !shield => {
            keys(&part, &["Defined", "CounterType", "CounterNum"])?;
            let counter = part.get("CounterType").and_then(counter_kind);
            match (counter, part.get("CounterNum")) {
                (Some(counter), Some("1")) => Ok(json!({"PreventRemovingCounter": counter})),
                _ => unmapped("replacement:DamageDone-counter"),
            }
        }
        ("ReplaceEffect", false) if part.get("VarName") == Some("Affected") && !shield => {
            keys(&part, &["VarName", "VarValue", "VarType"])?;
            redirect(p, &part)
        }
        ("ReplaceEffect", _) => unmapped("replacement:DamageDone-ReplaceEffect"),
        (_, true) => Ok(json!({"PreventAnd": [effect(ctx, &part, recipient_is_you, shield)?]})),
        (_, false) => Ok(json!({"Instead": [effect(ctx, &part, recipient_is_you, shield)?]})),
    }
}

/// Whether a part moves the `Effect` object from the command zone to exile, which ends it.
fn exiles_the_effect(part: &Params) -> bool {
    part.get("Defined") == Some("Self")
        && part.get("Origin") == Some("Command")
        && part.get("Destination") == Some("Exile")
}

/// The damage is dealt to the card with the ability or to what it is attached to, by the same source.
fn redirect(p: &Params, part: &Params) -> Tr<Value> {
    if part.get("VarType") != Some("Card") {
        return unmapped("replacement:DamageDone-redirect-VarType");
    }
    let to = match (p.get("DamageTarget"), part.get("VarValue")) {
        (Some("Self"), Some("Self")) => json!("This"),
        (Some("Enchanted"), Some("Enchanted")) | (Some("Equipped"), Some("Equipped")) => {
            json!("Attached")
        }
        _ => return unmapped("replacement:DamageDone-redirect-target"),
    };
    Ok(json!({"Instead": [{"DealDamage": {"source": "Dealer", "to": to, "amount": "Triggering"}}]}))
}
