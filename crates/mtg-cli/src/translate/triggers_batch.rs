//! Trigger modes that name an event with no new shape: "one or more" batches of discards, life loss, sacrifices, damage and
//! counters, a spell cast or copied, and an attacker that is not blocked.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::triggers::{keys, player_ref};
use super::triggers_more::{batch, counters, damage};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The pattern of a mode this module reads; `None` for any other mode.
pub fn pattern(ctx: &Ctx, p: &Params, mode: &str) -> Option<Tr<Value>> {
    Some(match mode {
        "AttackerUnblocked" => unblocked(ctx, p),
        "SpellCastOrCopy" => cast_or_copy(ctx, p),
        "DiscardedAll" => discarded(ctx, p).map(|inner| batch(&inner)),
        "LifeLostAll" => life_lost(p).map(|inner| batch(&inner)),
        "SacrificedOnce" => sacrificed(ctx, p).map(|inner| batch(&inner)),
        "DamageAll" => damage(ctx, p, "DamageAll").map(|inner| batch(&inner)),
        "DamageDealtOnce" => damage(ctx, p, "DamageDealtOnce").map(|inner| batch(&inner)),
        "UnlockDoor" => unlock_door(p),
        "CommitCrime" => player_event(p, "CommitCrime", "W11cCrime"),
        "DungeonCompleted" => player_event(p, "DungeonCompleted", "CompletedDungeon"),
        "RingTemptsYou" => ring_tempts(ctx, p),
        "BecomesCrewed" => becomes_crewed(ctx, p),
        "Attached" => attached(ctx, p),
        "CounterAddedOnce" | "CounterPlayerAddedAll" => {
            counters(ctx, p, "CountersPut").map(|inner| batch(&inner))
        }
        _ => return None,
    })
}

fn unblocked(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "AttackerUnblocked", &["ValidCard"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Creature"))?;
    Ok(json!({"Unblocked": {"what": what}}))
}

/// "Whenever you cast or copy a spell": either event triggers it alone.
fn cast_or_copy(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(
        p,
        "SpellCastOrCopy",
        &["ValidCard", "ValidActivatingPlayer"],
    )?;
    let by = player_ref(p.get("ValidActivatingPlayer"))?;
    let mut cast = json!({ "by": by });
    let mut copy = json!({ "by": by });
    if let Some(valid) = p.get("ValidCard") {
        cast["what"] = json!(selector_any(ctx, valid)?);
        copy["what"] = cast["what"].clone();
    }
    Ok(json!({"AnyOf": [{"SpellCast": cast}, {"SpellCopied": copy}]}))
}

fn discarded(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "DiscardedAll", &["ValidCard", "ValidPlayer"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Card"))?;
    let by = player_ref(p.get("ValidPlayer"))?;
    let mut pattern = json!({ "by": by });
    if !what.is_empty() {
        pattern["what"] = json!(what);
    }
    Ok(json!({ "Discarded": pattern }))
}

fn life_lost(p: &Params) -> Tr<Value> {
    keys(p, "LifeLostAll", &["ValidPlayer"])?;
    let who = player_ref(p.get("ValidPlayer"))?;
    Ok(json!({"LifeLost": {"who": who}}))
}

fn sacrificed(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "SacrificedOnce", &["ValidCard", "ValidPlayer"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Card"))?;
    let by = player_ref(p.get("ValidPlayer"))?;
    Ok(json!({"Sacrificed": {"what": what, "by": by}}))
}

/// "When you unlock this door": the door of the card the ability is printed on, set as the card is compiled.
fn unlock_door(p: &Params) -> Tr<Value> {
    keys(p, "UnlockDoor", &["ValidPlayer", "ValidCard", "ThisDoor"])?;
    if p.get("ThisDoor") != Some("True") || p.get("ValidCard") != Some("Card.Self") {
        return unmapped("key:Trigger.UnlockDoor.ThisDoor");
    }
    let who = player_ref(p.get("ValidPlayer"))?;
    Ok(json!({"DoorUnlocked": {"who": who, "door": 0}}))
}

/// A player does something no object is part of: the mode's `ValidPlayer` is the player.
fn player_event(p: &Params, mode: &str, variant: &str) -> Tr<Value> {
    keys(p, mode, &["ValidPlayer"])?;
    let who = player_ref(p.get("ValidPlayer"))?;
    Ok(json!({ variant: {"who": who} }))
}

/// The Ring tempts a player: `ValidPlayer` is who; `ValidCard` is the creature chosen as Ring-bearer, which only its
/// controller's Ring can choose, so it names the player when `ValidPlayer` is absent.
fn ring_tempts(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "RingTemptsYou", &["ValidPlayer", "ValidCard"])?;
    let mut pattern = json!({});
    let who = match (p.get("ValidPlayer"), p.get("ValidCard")) {
        (None, Some(valid)) if valid.contains("YouCtrl") => json!("You"),
        (None, Some(_)) => return unmapped("key:Trigger.RingTemptsYou.ValidCard"),
        (valid, _) => player_ref(valid)?,
    };
    pattern["who"] = who;
    if let Some(valid) = p.get("ValidCard") {
        pattern["bearer"] = json!(selector_any(ctx, valid)?);
    }
    Ok(json!({"RingTempts": pattern}))
}

/// A Vehicle becomes crewed, the first time in a turn when the line says so.
fn becomes_crewed(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "BecomesCrewed", &["ValidCard", "FirstTimeCrewed"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Vehicle"))?;
    let event = json!({"BecomesCrewed": {"what": what}});
    match p.get("FirstTimeCrewed") {
        None => Ok(event),
        Some("True") => Ok(json!({"Nth": {"event": event, "nth": 1}})),
        Some(other) => unmapped(format!("key:Trigger.BecomesCrewed.FirstTimeCrewed={other}")),
    }
}

/// An Aura, Equipment or Fortification becomes attached: `ValidSource` is the attachment, `ValidTarget` what it joins.
fn attached(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "Attached", &["ValidSource", "ValidTarget"])?;
    let mut pattern = json!({});
    for (key, field) in [("ValidSource", "what"), ("ValidTarget", "to")] {
        if let Some(valid) = p.get(key) {
            pattern[field] = json!(selector_any(ctx, valid)?);
        }
    }
    Ok(json!({ "Attached": pattern }))
}
