//! Damage prevention: a `Event$ DamageDone | Prevent$ True` replacement as a static `Replacement` of a permanent, or as the
//! `Shield` that Forge's `Effect` object (`ReplacementEffects$`) makes for a while.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, is_hint, unmapped};
use super::effects_obj::duration;
use super::valid::{selector_any, subject};
use crate::forge::script::Params;

const KEYS: [&str; 8] = [
    "ReplaceWith",
    "Event",
    "Prevent",
    "IsCombat",
    "ValidSource",
    "ValidTarget",
    "ActiveZones",
    "Secondary",
];

/// What a `ValidSource$` or `ValidTarget$` word may name besides plain valid-strings: the objects an `Effect` remembers.
fn is_remembered(word: &str) -> bool {
    word.split(',').all(|part| {
        matches!(
            part.trim(),
            "Card.IsRemembered" | "Creature.IsRemembered" | "Player.IsRemembered"
        )
    })
}

/// The `Event$ DamageDone` keys all of which are understood, with the zone the line is active in.
fn read_keys(p: &Params, static_line: bool) -> Tr<()> {
    for (key, _) in p.iter() {
        if !KEYS.contains(&key) && !is_hint(key) {
            return unmapped(format!("key:Replacement.DamageDone.{key}"));
        }
    }
    let prevents = p.get("Prevent") == Some("True");
    let replaces = p.has("ReplaceWith");
    if prevents == replaces || (replaces && !static_line) {
        return unmapped("replacement:DamageDone-not-a-prevention");
    }
    match (p.get("ActiveZones"), static_line) {
        (None | Some("Battlefield"), true) | (None | Some("Command"), false) => Ok(()),
        _ => unmapped("replacement:DamageDone-zone"),
    }
}

/// Whether the damage is combat damage only, noncombat only, or either.
fn combat(p: &Params) -> Tr<Option<bool>> {
    match p.get("IsCombat") {
        None => Ok(None),
        Some("True") => Ok(Some(true)),
        Some("False") => Ok(Some(false)),
        Some(_) => unmapped("key:Replacement.DamageDone.IsCombat"),
    }
}

/// The recipient of a line that stays on a permanent, which is read each time damage would be dealt.
fn static_recipient(ctx: &Ctx, word: &str) -> Tr<Value> {
    Ok(match word {
        "You" => json!("You"),
        "Player" => json!("EachPlayer"),
        "Opponent" | "Player.Opponent" => json!("Opponents"),
        "Card.Self" | "Creature.Self" => json!("This"),
        "Card.AttachedBy" | "Creature.AttachedBy" | "Card.EnchantedBy" | "Creature.EnchantedBy"
        | "Creature.EquippedBy" | "Card.EquippedBy" => json!("Attached"),
        other if other.contains(',') || is_remembered(other) => {
            return unmapped("replacement:DamageDone-recipient-list");
        }
        other => json!({"Each": selector_any(ctx, other)?}),
    })
}

/// A prevention line of a permanent as a `Replacement`.
pub fn static_prevention(ctx: &Ctx, p: &Params) -> Tr<Value> {
    read_keys(p, true)?;
    let mut when = serde_json::Map::new();
    if let Some(word) = p.get("ValidTarget") {
        when.insert("to".into(), static_recipient(ctx, word)?);
    }
    if let Some(word) = p.get("ValidSource") {
        if is_remembered(word) {
            return unmapped("replacement:DamageDone-remembered-in-a-static");
        }
        when.insert("source".into(), json!(selector_any(ctx, word)?));
    }
    if let Some(combat) = combat(p)? {
        when.insert("combat".into(), json!(combat));
    }
    let by = match p.get("ReplaceWith") {
        None => json!("Prevent"),
        Some(name) => scaled(ctx, name)?,
    };
    Ok(json!({"Replacement": {"when": {"Damage": when}, "by": by}}))
}

/// `ReplaceEffect` of the damage amount (`ReplaceCount$DamageAmount/Twice`) as a multiplication or an addition.
fn scaled(ctx: &Ctx, name: &str) -> Tr<Value> {
    let Some(part) = ctx.svar(name).map(Params::parse) else {
        return unmapped("svar:missing");
    };
    for (key, _) in part.iter() {
        if !["DB", "VarName", "VarValue"].contains(&key) && !is_hint(key) {
            return unmapped(format!("key:ReplaceEffect.{key}"));
        }
    }
    if part.get("DB") != Some("ReplaceEffect") || part.get("VarName") != Some("DamageAmount") {
        return unmapped("replacement:DamageDone-ReplaceWith-shape");
    }
    let expression = part.get("VarValue").and_then(|var| ctx.svar(var)).unwrap_or_default();
    match expression.strip_prefix("ReplaceCount$DamageAmount/") {
        Some("Twice") => Ok(json!({"Multiply": 2})),
        Some("Thrice") => Ok(json!({"Multiply": 3})),
        Some(step) => match step.strip_prefix("Plus.").and_then(|n| n.parse::<u32>().ok()) {
            Some(n) => Ok(json!({"Add": n})),
            None => unmapped("replacement:DamageDone-ReplaceCount-step"),
        },
        None => unmapped("replacement:DamageDone-ReplaceCount"),
    }
}

/// The `Shield` an `Effect` line makes from the prevention its `ReplacementEffects$` names; `remembered` is what
/// `RememberObjects$` stands for.
pub fn shield(ctx: &mut Ctx, line: &Params, remembered: Option<Value>) -> Tr<Vec<Value>> {
    let Some(name) = line.get("ReplacementEffects") else {
        return unmapped("key:Effect.no-ReplacementEffects");
    };
    if name.contains(',') {
        return unmapped("key:Effect.ReplacementEffects-list");
    }
    let p = ctx.sub(name)?;
    if p.get("Event") != Some("DamageDone") {
        return unmapped(format!(
            "replacement:{}",
            p.get("Event").unwrap_or("no-Event")
        ));
    }
    read_keys(&p, false)?;
    let duration = duration(line.get("Duration"))?;
    let mut when = serde_json::Map::new();
    match (p.get("ValidTarget"), &remembered) {
        (Some(word), Some(who)) if is_remembered(word) => {
            when.insert("to".into(), who.clone());
        }
        (Some(word), _) => {
            let to = match word {
                "You" => json!("You"),
                "Player" => json!("EachPlayer"),
                "Opponent" | "Player.Opponent" => json!("Opponents"),
                "Card.EffectSource" | "Creature.EffectSource" => json!("This"),
                _ => return unmapped("replacement:DamageDone-shield-recipient"),
            };
            when.insert("to".into(), to);
        }
        (None, _) => {}
    }
    match (p.get("ValidSource"), &remembered) {
        (Some(word), Some(who)) if is_remembered(word) => {
            when.insert("source".into(), json!([{"Is": who}]));
        }
        (Some(word), _) if word.contains("Remembered") || word.contains("Chosen") => {
            return unmapped("replacement:DamageDone-shield-source");
        }
        (Some(word), _) => {
            when.insert("source".into(), json!(selector_any(ctx, word)?));
        }
        (None, _) => {}
    }
    if let Some(combat) = combat(&p)? {
        when.insert("combat".into(), json!(combat));
    }
    Ok(vec![
        json!({"Shield": {"when": {"Damage": when}, "by": "Prevent", "duration": duration}}),
    ])
}

/// The recipient and the remembered objects of an `Effect` line that has a target.
pub fn remembered_of(ctx: &mut Ctx, line: &Params) -> Tr<Option<Value>> {
    match line.get("RememberObjects") {
        None if line.has("ValidTgts") => unmapped("key:Effect.target-not-remembered"),
        None => Ok(None),
        Some("Targeted") if line.has("ValidTgts") => Ok(Some(subject(ctx, line, "", None)?)),
        Some("Targeted") => Ok(Some(ctx.last_target()?)),
        Some("Self") => Ok(Some(json!("This"))),
        Some(other) => unmapped(format!("key:Effect.RememberObjects={other}")),
    }
}
