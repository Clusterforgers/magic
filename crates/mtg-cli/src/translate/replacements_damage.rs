//! Damage prevention: a `Event$ DamageDone | Prevent$ True` replacement as a static `Replacement` of a permanent, or as the
//! `Shield` that Forge's `Effect` object (`ReplacementEffects$`) makes for a while.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::effects_obj::duration;
use super::valid::{selector_any, source_any, subject};
use crate::forge::script::Params;

const KEYS: [&str; 10] = [
    "PreventionEffect",
    "DamageTarget",
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

/// Whether a `ValidSource$` word names only the source `ChooseSource` picked: `Card.ChosenCardStrict,Emblem.ChosenCard`,
/// or a card that is the chosen one and has a colour or type (`Card.ChosenCardStrict+RedSource`), which the choice
/// already limited to, so the shield holds the chosen object alone (it keeps its choice if it changes colour, CR 609.7).
fn chosen_source(word: &str) -> bool {
    word.split(',').all(|alternative| {
        let alternative = alternative.trim();
        let tail = alternative
            .strip_prefix("Card.")
            .or_else(|| alternative.strip_prefix("Emblem."));
        let mut parts = tail.unwrap_or_default().split('+');
        matches!(parts.next(), Some("ChosenCard" | "ChosenCardStrict"))
            && parts.all(|part| part.ends_with("Source") || matches!(part, "Artifact" | "Land"))
    })
}

/// The `Event$ DamageDone` keys all of which are understood, with the zone the line is active in.
fn read_keys(p: &Params, static_line: bool) -> Tr<()> {
    check_keys(p, "Replacement.DamageDone", &KEYS, |_, _| false)?;
    if p.has("DamageTarget") && !p.has("ReplaceWith") {
        return unmapped("key:Replacement.DamageDone.DamageTarget");
    }
    let prevents = p.get("Prevent") == Some("True");
    let replaces = p.has("ReplaceWith");
    let shield_prevention = p.get("PreventionEffect") == Some("True");
    if prevents == replaces || (replaces && !static_line && !shield_prevention) {
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
        "Card.AttachedBy"
        | "Creature.AttachedBy"
        | "Card.EnchantedBy"
        | "Creature.EnchantedBy"
        | "Creature.EquippedBy"
        | "Card.EquippedBy" => json!("Attached"),
        other if other.contains(',') || is_remembered(other) => {
            return unmapped("replacement:DamageDone-recipient-list");
        }
        other => json!({"Each": selector_any(ctx, other)?}),
    })
}

/// A prevention line of a permanent as a `Replacement`.
pub fn static_prevention(ctx: &Ctx, p: &Params) -> Tr<Vec<Value>> {
    read_keys(p, true)?;
    let you = p.get("ValidTarget") == Some("You");
    let by = match p.get("ReplaceWith") {
        None => json!("Prevent"),
        Some(name) if is_damage_scaling(ctx, name) => scaled(ctx, name)?,
        Some(name) => super::replacements_damage_by::by(ctx, p, name, you, false)?,
    };
    // A list of one player and one permanent word names disjoint recipients, so one replacement for each is exact.
    let mut out = Vec::new();
    for recipient in recipients(ctx, p.get("ValidTarget"))? {
        out.push(static_replacement(ctx, p, recipient, &by)?);
    }
    Ok(out)
}

/// Whether a `ReplaceWith$` part changes the damage amount (`ReplaceEffect` of `DamageAmount`).
fn is_damage_scaling(ctx: &Ctx, name: &str) -> bool {
    ctx.svar(name)
        .map(Params::parse)
        .is_some_and(|part| part.get("VarName") == Some("DamageAmount"))
}

/// The recipients a line is about, one replacement for each: none for "any permanent or player".
fn recipients(ctx: &Ctx, word: Option<&str>) -> Tr<Vec<Option<Value>>> {
    let Some(word) = word else {
        return Ok(vec![None]);
    };
    if !word.contains(',') {
        return Ok(vec![Some(static_recipient(ctx, word)?)]);
    }
    let parts: Vec<&str> = word.split(',').map(str::trim).collect();
    let is_player = |part: &str| matches!(part, "You" | "Player" | "Opponent" | "Player.Opponent");
    match parts.as_slice() {
        ["Permanent", "Player"] | ["Player", "Permanent"] => Ok(vec![None]),
        [one, other] if is_player(one) != is_player(other) => Ok(vec![
            Some(static_recipient(ctx, one)?),
            Some(static_recipient(ctx, other)?),
        ]),
        _ => unmapped("replacement:DamageDone-recipient-list"),
    }
}

/// One `Replacement` of damage to `recipient` by the line's source and combat restrictions.
fn static_replacement(ctx: &Ctx, p: &Params, recipient: Option<Value>, by: &Value) -> Tr<Value> {
    let mut when = serde_json::Map::new();
    if let Some(to) = recipient {
        when.insert("to".into(), to);
    }
    if let Some(word) = p.get("ValidSource") {
        if is_remembered(word) {
            return unmapped("replacement:DamageDone-remembered-in-a-static");
        }
        when.insert("source".into(), json!(source_any(ctx, word)?));
    }
    if let Some(combat) = combat(p)? {
        when.insert("combat".into(), json!(combat));
    }
    Ok(json!({"Replacement": {"when": {"Damage": when}, "by": by}}))
}

/// `ReplaceEffect` of the damage amount (`ReplaceCount$DamageAmount/Twice`) as a multiplication or an addition.
fn scaled(ctx: &Ctx, name: &str) -> Tr<Value> {
    let Some(part) = ctx.svar(name).map(Params::parse) else {
        return unmapped("svar:missing");
    };
    check_keys(
        &part,
        "ReplaceEffect",
        &["DB", "VarName", "VarValue"],
        |_, _| false,
    )?;
    if part.get("DB") != Some("ReplaceEffect") || part.get("VarName") != Some("DamageAmount") {
        return unmapped("replacement:DamageDone-ReplaceWith-shape");
    }
    let expression = part
        .get("VarValue")
        .and_then(|var| ctx.svar(var))
        .unwrap_or_default();
    match expression.strip_prefix("ReplaceCount$DamageAmount/") {
        Some("Twice") => Ok(json!({"Multiply": 2})),
        Some("Thrice") => Ok(json!({"Multiply": 3})),
        Some(step) => match step
            .strip_prefix("Plus.")
            .and_then(|n| n.parse::<u32>().ok())
        {
            Some(n) => Ok(json!({"Add": n})),
            None => unmapped("replacement:DamageDone-ReplaceCount-step"),
        },
        None => unmapped("replacement:DamageDone-ReplaceCount"),
    }
}

/// The `Shield` an `Effect` line makes from the prevention its `ReplacementEffects$` names; `remembered` is what
/// `RememberObjects$` stands for.
pub fn shield(ctx: &mut Ctx, line: &Params, remembered: Option<&Value>) -> Tr<Vec<Value>> {
    let Some(name) = line.get("ReplacementEffects") else {
        return unmapped("key:Effect.no-ReplacementEffects");
    };
    let mut out = Vec::new();
    for name in name.split(',').map(str::trim) {
        out.extend(one_shield(ctx, line, name, remembered)?);
    }
    Ok(out)
}

/// The `Shield` of one named replacement of an `Effect` line.
fn one_shield(
    ctx: &mut Ctx,
    line: &Params,
    name: &str,
    remembered: Option<&Value>,
) -> Tr<Vec<Value>> {
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
    match (p.get("ValidTarget"), remembered) {
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
    match (p.get("ValidSource"), remembered) {
        (Some(word), Some(who)) if is_remembered(word) => {
            when.insert("source".into(), json!([{"Is": who}]));
        }
        (Some(word), _) if chosen_source(word) => {
            let Some(name) = &ctx.chosen_card else {
                return unmapped("valid:ChosenCard-without-a-choice");
            };
            when.insert("source".into(), json!([{"Is": {"Bound": name}}]));
        }
        (Some(word), _) if word.contains("Remembered") || word.contains("Chosen") => {
            return unmapped("replacement:DamageDone-shield-source");
        }
        (Some(word), _) => {
            when.insert("source".into(), json!(source_any(ctx, word)?));
        }
        (None, _) => {}
    }
    if let Some(combat) = combat(&p)? {
        when.insert("combat".into(), json!(combat));
    }
    let by = match p.get("ReplaceWith") {
        None => json!("Prevent"),
        Some(_) if p.get("Prevent") == Some("True") => {
            return unmapped("replacement:DamageDone-prevent-and-replace");
        }
        Some(name) => {
            let you = p.get("ValidTarget") == Some("You");
            super::replacements_damage_by::by(ctx, &p, name, you, true)?
        }
    };
    Ok(vec![
        json!({"Shield": {"when": {"Damage": when}, "by": by, "duration": duration}}),
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
