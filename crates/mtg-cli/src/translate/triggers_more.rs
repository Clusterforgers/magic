//! More trigger modes: batches ("one or more"), mana, targeting, counters, life loss and the nth spell of a turn.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::triggers::{keys, player_ref};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The pattern of a mode this module reads; `None` for any other mode.
pub fn pattern(ctx: &Ctx, p: &Params, mode: &str) -> Option<Tr<Value>> {
    Some(match mode {
        "DamageDoneOnce" => damage(ctx, p, "DamageDoneOnce").map(|inner| batch(&inner)),
        "ChangesZoneAll" => zone_changes(ctx, p).map(|inner| batch(&inner)),
        "LifeLost" => life_lost(p),
        "TapsForMana" => taps_for_mana(ctx, p),
        "BecomesTarget" => becomes_target(ctx, p),
        "CounterAdded" => counters(ctx, p, "CountersPut"),
        "CounterRemoved" => counters(ctx, p, "CountersRemoved"),
        _ => return None,
    })
}

/// One or more events together trigger the ability once.
pub(super) fn batch(inner: &Value) -> Value {
    json!({ "Batch": inner })
}

/// The recipient of damage: a player word, the card itself, what it enchants or equips, or the objects a valid-string names.
fn recipient(ctx: &Ctx, valid: &str) -> Tr<Value> {
    Ok(match valid {
        "Player" | "You" | "Opponent" => super::triggers::player_ref(Some(valid))?,
        word if word.starts_with("Player.") => super::triggers::player_ref(Some(word))?,
        "Card.Self" | "Creature.Self" => json!("This"),
        "Card.AttachedBy" | "Creature.AttachedBy" | "Card.EnchantedBy" | "Creature.EnchantedBy" => {
            json!("Attached")
        }
        other => json!({"Each": selector_any(ctx, other)?}),
    })
}

/// `DamageDone` and `DamageDoneOnce`: a source deals damage to a recipient.
pub fn damage(ctx: &Ctx, p: &Params, mode: &str) -> Tr<Value> {
    keys(p, mode, &["ValidSource", "ValidTarget", "CombatDamage"])?;
    let source = super::valid::source_any(ctx, p.get("ValidSource").unwrap_or("Card"))?;
    let mut pattern = json!({ "source": source });
    if p.get("CombatDamage") == Some("True") {
        pattern["combat"] = json!(true);
    }
    let Some(target) = p.get("ValidTarget") else {
        return Ok(json!({ "DealsDamage": pattern }));
    };
    // "To a player or a planeswalker": one pattern for each recipient, any of which triggers.
    let recipients: Vec<&str> = target.split(',').collect();
    if let [only] = recipients.as_slice() {
        pattern["to"] = recipient(ctx, only)?;
        return Ok(json!({ "DealsDamage": pattern }));
    }
    if mode != "DamageDone" {
        return unmapped("key:Trigger.DamageDoneOnce.ValidTarget-or");
    }
    let mut any = Vec::new();
    for each in recipients {
        let mut one = pattern.clone();
        one["to"] = recipient(ctx, each)?;
        any.push(json!({ "DealsDamage": one }));
    }
    Ok(json!({ "AnyOf": any }))
}

/// `ChangesZoneAll`: cards change zones together.
fn zone_changes(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(
        p,
        "ChangesZoneAll",
        &["ValidCards", "Origin", "Destination"],
    )?;
    let what = selector_any(ctx, p.get("ValidCards").unwrap_or("Card"))?;
    let mut pattern = json!({ "what": what });
    for (key, field) in [("Origin", "from"), ("Destination", "to")] {
        match p.get(key) {
            None | Some("Any") => {}
            Some(zone @ ("Battlefield" | "Graveyard" | "Hand" | "Library" | "Exile" | "Stack")) => {
                pattern[field] = json!(zone);
            }
            Some(other) => return unmapped(format!("zone:Trigger.{key}={other}")),
        }
    }
    Ok(json!({ "ZoneChange": pattern }))
}

fn life_lost(p: &Params) -> Tr<Value> {
    keys(p, "LifeLost", &["ValidPlayer"])?;
    let who = player_ref(p.get("ValidPlayer"))?;
    Ok(json!({"LifeLost": {"who": who}}))
}

fn taps_for_mana(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "TapsForMana", &["ValidCard", "Activator", "Static"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Permanent"))?;
    let by = match p.get("Activator") {
        None => "EachPlayer",
        Some("You") => "You",
        Some("Opponent" | "Player.Opponent") => "Opponents",
        Some(other) => return unmapped(format!("key:Trigger.TapsForMana.Activator={other}")),
    };
    Ok(json!({"TappedForMana": {"what": what, "by": by}}))
}

fn becomes_target(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(
        p,
        "BecomesTarget",
        &["ValidTarget", "ValidSource", "FirstTime"],
    )?;
    // The pattern holds objects: a player that becomes a target is another event.
    if p.get("ValidTarget").is_some_and(|valid| {
        valid.split(',').any(|word| {
            let head = word.split('.').next().unwrap_or_default();
            matches!(head, "You" | "Player" | "Opponent")
        })
    }) {
        return unmapped("trigger:BecomesTarget-a-player");
    }
    let what = selector_any(ctx, p.get("ValidTarget").unwrap_or("Card"))?;
    let by = match p.get("ValidSource") {
        Some(valid) => super::valid::source_any(ctx, valid)?,
        None => Vec::new(),
    };
    // The turn's count of targetings counts every source, so only a line with no source word can ask for the first.
    if p.get("FirstTime") == Some("True") && !by.is_empty() {
        return unmapped("key:Trigger.BecomesTarget.FirstTime-with-source");
    }
    super::triggers::first_time(p, json!({"BecomesTarget": {"what": what, "by": by}}))
}

/// `CounterAdded` and `CounterRemoved` of the kind `CounterType$` names.
pub(super) fn counters(ctx: &Ctx, p: &Params, variant: &str) -> Tr<Value> {
    keys(p, "Counter", &["ValidCard", "CounterType"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Permanent"))?;
    let mut pattern = json!({ "what": what });
    if let Some(kind) = p.get("CounterType") {
        let Some(kind) = super::amount::counter_kind(kind) else {
            return unmapped("key:Trigger.Counter.CounterType");
        };
        pattern["counter"] = kind;
    }
    Ok(json!({ variant: pattern }))
}

/// The nth spell of a turn: `ActivatorThisTurnCast$ EQn`.
pub fn nth_spell(p: &Params, pattern: Value) -> Tr<Value> {
    match p.get("ActivatorThisTurnCast") {
        None => Ok(pattern),
        Some(word) => match word.strip_prefix("EQ").and_then(|n| n.parse::<u32>().ok()) {
            Some(nth) => Ok(json!({"Nth": {"event": pattern, "nth": nth}})),
            None => unmapped("key:Trigger.SpellCast.ActivatorThisTurnCast"),
        },
    }
}
