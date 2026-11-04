//! Trigger modes that name an event with no new shape: "one or more" batches of discards, life loss, sacrifices, damage and
//! counters, a spell cast or copied, and an attacker that is not blocked.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr};
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
        "CounterAddedOnce" => counters(ctx, p, "CountersPut").map(|inner| batch(&inner)),
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
