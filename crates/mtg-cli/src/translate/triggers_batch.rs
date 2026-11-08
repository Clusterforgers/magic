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
        "DiscardedAll" if p.get("FirstTime").is_some() => first_discards(ctx, p),
        "DiscardedAll" => discarded(ctx, p).map(|inner| batch(&inner)),
        "Milled" => milled(ctx, p, false),
        "MilledAll" => milled(ctx, p, false).map(|inner| batch(&inner)),
        "MilledOnce" => milled(ctx, p, true).map(|inner| batch(&inner)),
        "LifeLostAll" => life_lost(p).map(|inner| batch(&inner)),
        "SacrificedOnce" => sacrificed(ctx, p).map(|inner| batch(&inner)),
        "DamageAll" => damage(ctx, p, "DamageAll").map(|inner| batch(&inner)),
        "DamageDealtOnce" => damage(ctx, p, "DamageDealtOnce").map(|inner| per_source(&inner)),
        "UnlockDoor" => unlock_door(p),
        "CommitCrime" => player_event(p, "CommitCrime", "W11cCrime"),
        "DungeonCompleted" => player_event(p, "DungeonCompleted", "CompletedDungeon"),
        "RingTemptsYou" => ring_tempts(ctx, p),
        "BecomesCrewed" => becomes_crewed(ctx, p),
        "Attached" => attached(ctx, p),
        "CounterAddedOnce" if p.has("ValidPlayer") && !p.has("ValidCard") => {
            player_counters(p).map(|inner| batch(&inner))
        }
        "CounterTypeAddedAll" => counters(ctx, &p.without(&["FirstTime"]), "CountersPut")
            .and_then(|inner| {
                // The engine counts the first counters put on an object in a turn as the event itself, not a batch.
                let first = p.get("FirstTime") == Some("True");
                super::triggers::first_time(p, if first { inner } else { batch(&inner) })
            }),
        "CounterAddedOnce" | "CounterPlayerAddedAll" => {
            counters(ctx, p, "CountersPut").map(|inner| {
                let mut found = batch(&inner);
                e19_each_by_wording(&mut found, p.get("TriggerDescription"));
                found
            })
        }
        _ => return None,
    })
}

/// `DamageDealtOnce` triggers once for each source: a group of sources with no recipient named is the plain event of
/// each creature, one object (or a named recipient) a batch of what it deals together.
fn per_source(inner: &Value) -> Value {
    // Only a group named by kind and controller is the plain event of each of its creatures.
    let by_kind = |source: &Value| {
        source.as_array().is_some_and(|preds| {
            preds.iter().all(|pred| {
                pred == "Spell"
                    || pred.as_object().is_some_and(|map| {
                        map.keys()
                            .all(|key| matches!(key.as_str(), "Type" | "Subtype" | "ControlledBy"))
                    })
            })
        })
    };
    let one = inner
        .get("DealsDamage")
        .is_none_or(|event| event.get("to").is_some() || !by_kind(&event["source"]));
    if one { batch(inner) } else { inner.clone() }
}

/// "Whenever you get one or more {E}": counters put on a player together (e16c).
fn player_counters(p: &Params) -> Tr<Value> {
    keys(p, "PlayerCounter", &["ValidPlayer", "CounterType"])?;
    let mut pattern = json!({ "who": player_ref(p.get("ValidPlayer"))? });
    if let Some(kind) = p.get("CounterType") {
        let Some(kind) = super::amount::counter_kind(kind) else {
            return unmapped("key:Trigger.PlayerCounter.CounterType");
        };
        pattern["counter"] = kind;
    }
    Ok(json!({ "PlayerCountersPut": pattern }))
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

/// "Whenever you discard one or more cards for the first time each turn": the batch of the player's first discards. A
/// first time among the cards of a kind (Veronica's nonland cards) needs the kind of every earlier discard and stays a gap.
fn first_discards(ctx: &Ctx, p: &Params) -> Tr<Value> {
    if p.get("ValidCard").is_some_and(|valid| valid != "Card") {
        return unmapped("key:Trigger.DiscardedAll.FirstTime-of-a-kind");
    }
    let line = p.without(&["FirstTime"]);
    let inner = discarded(ctx, &line)?;
    super::triggers::first_time(p, batch(&inner))
}

/// Cards milled: `ValidPlayer` is whose library they come from; `MilledOnce` batches them for each player.
fn milled(ctx: &Ctx, p: &Params, per_player: bool) -> Tr<Value> {
    keys(p, "Milled", &["ValidCard", "ValidPlayer"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Card"))?;
    let by = player_ref(p.get("ValidPlayer"))?;
    let mut pattern = json!({ "what": what, "by": by });
    if per_player {
        pattern["per_player"] = json!(true);
    }
    Ok(json!({ "Milled": pattern }))
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

/// Marks a batch of counters put on permanents `each` when its description names a single permanent ("one or more -1/-1
/// counters are put on a creature", "you put one or more counters on another creature"): the counters put on each are an
/// event of their own (CR 603.2c). A plural subject ("on one or more creatures you control") and the card itself stay one
/// event.
pub fn e19_each_by_wording(pattern: &mut Value, description: Option<&str>) {
    let Some(put) = pattern
        .get_mut("Batch")
        .and_then(|batch| batch.get_mut("CountersPut"))
    else {
        return;
    };
    let itself = put
        .get("what")
        .and_then(Value::as_array)
        .is_some_and(|what| what.iter().any(|pred| pred == "This"));
    if itself {
        return;
    }
    let text = description.unwrap_or_default().to_lowercase();
    let singular = ["counters are put on ", "counters on "].iter().any(|said| {
        text.split(said).skip(1).any(|rest| {
            ["a ", "an ", "another "]
                .iter()
                .any(|article| rest.starts_with(article))
        })
    });
    if singular {
        put["each"] = json!(true);
    }
}
