//! Triggers: a `T:` line as the DSL's `Triggered` ability, its event pattern and the effects of its `Execute$` part.

use serde_json::{Value, json};

use super::condition::{condition, strip};
use super::ctx::{Ctx, Tr, unmapped};
use super::effects::chain_at;
use super::valid::selector_any;
use crate::forge::script::Params;

/// The keys every trigger line may carry besides its mode's own.
const COMMON: [&str; 4] = ["Mode", "Execute", "TriggerZones", "OptionalDecider"];

/// A trigger line as a `Triggered` ability.
pub fn triggered(ctx: &mut Ctx, body: &str) -> Tr<Value> {
    ctx.reset();
    let all = Params::parse(body);
    let intervening = condition(ctx, &all)?;
    let p = strip(&all);
    let when = pattern(ctx, &p)?;
    let from = match p.get("TriggerZones") {
        None | Some("Battlefield") => None,
        Some(zone @ ("Graveyard" | "Hand" | "Exile" | "Library")) => Some(zone),
        Some(other) => return unmapped(format!("key:Trigger.TriggerZones={other}")),
    };
    let Some(execute) = p.get("Execute") else {
        return unmapped("trigger:no-Execute");
    };
    let part = ctx.sub(execute)?;
    let mut effects = chain_at(ctx, &part, false)?;
    let itself = is_about_itself(&when);
    if itself {
        for effect in &mut effects {
            about_this(effect);
        }
    }
    // Paying a cost is a choice already: no "you may" beside "you may pay".
    let pays = matches!(effects.as_slice(), [only] if only.get("Pay").is_some());
    match p.get("OptionalDecider") {
        None => {}
        Some("You" | "TriggeredCardController") if pays => {}
        Some("You") => effects = vec![json!({"May": {"effects": effects}})],
        Some("TriggeredCardController") if itself => {
            effects = vec![json!({"May": {"effects": effects}})];
        }
        Some(other) => return unmapped(format!("key:Trigger.OptionalDecider={other}")),
    }
    let mut ability = json!({"when": when, "effects": effects});
    if let Some(zone) = from {
        ability["from"] = json!(zone);
    }
    if let Some(condition) = intervening {
        ability["if"] = condition;
    }
    if !ctx.targets.is_empty() {
        ability["targets"] = Value::Array(ctx.targets.clone());
    }
    Ok(json!({ "Triggered": ability }))
}

/// Whether a pattern is about the card itself ("when this creature dies"), so that "that creature" is the card and its
/// controller is "you".
fn is_about_itself(when: &Value) -> bool {
    when.as_object()
        .and_then(|map| map.values().next())
        .and_then(|inner| inner.get("what"))
        .is_some_and(|what| *what == json!(["This"]))
}

/// Rewrites the references to the triggering object in the effects of a trigger about itself.
fn about_this(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if map.len() == 1 && map.get("ControllerOf") == Some(&json!("Triggering")) {
                *value = json!("You");
                return;
            }
            for inner in map.values_mut() {
                about_this(inner);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(about_this),
        Value::String(word) if word == "Triggering" => *value = json!("This"),
        _ => {}
    }
}

/// Checks the keys of a trigger line against those a mode reads.
fn keys(p: &Params, mode: &str, read: &[&str]) -> Tr<()> {
    for (key, _) in p.iter() {
        if COMMON.contains(&key) || read.contains(&key) || super::ctx::is_hint(key) {
            continue;
        }
        return unmapped(format!("key:Trigger.{mode}.{key}"));
    }
    Ok(())
}

fn pattern(ctx: &Ctx, p: &Params) -> Tr<Value> {
    let mode = p.get("Mode").unwrap_or_default();
    match mode {
        "ChangesZone" => changes_zone(ctx, p),
        "Phase" => phase(p),
        "Attacks" => attacks(ctx, p),
        "SpellCast" => spell_cast(ctx, p),
        "DamageDone" => damage_done(ctx, p),
        "Drawn" => drawn(p),
        "LifeGained" => life_gained(p),
        "Sacrificed" => what_trigger(ctx, p, "Sacrificed"),
        "Taps" => what_trigger(ctx, p, "Tapped"),
        "Untaps" => what_trigger(ctx, p, "Untapped"),
        "Blocks" => what_trigger(ctx, p, "Blocks"),
        "AttackerBlocked" => what_trigger(ctx, p, "BecomesBlocked"),
        other => unmapped(format!("trigger:{other}")),
    }
}

/// A zone word of a trigger (`Any` is no zone).
fn zone(word: Option<&str>, key: &str) -> Tr<Option<&'static str>> {
    match word {
        None | Some("Any") => Ok(None),
        Some("Battlefield") => Ok(Some("Battlefield")),
        Some("Graveyard") => Ok(Some("Graveyard")),
        Some("Hand") => Ok(Some("Hand")),
        Some("Library") => Ok(Some("Library")),
        Some("Exile") => Ok(Some("Exile")),
        Some("Stack") => Ok(Some("Stack")),
        Some(other) => unmapped(format!("zone:Trigger.{key}={other}")),
    }
}

fn changes_zone(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "ChangesZone", &["ValidCard", "Origin", "Destination"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Card"))?;
    let mut pattern = json!({ "what": what });
    if let Some(from) = zone(p.get("Origin"), "Origin")? {
        pattern["from"] = json!(from);
    }
    if let Some(to) = zone(p.get("Destination"), "Destination")? {
        pattern["to"] = json!(to);
    }
    Ok(json!({ "ZoneChange": pattern }))
}

fn step(word: &str) -> Tr<&'static str> {
    Ok(match word {
        "Upkeep" => "Upkeep",
        "Draw" => "Draw",
        "Main1" => "Main1",
        "Main2" => "Main2",
        "BeginCombat" => "BeginCombat",
        "EndCombat" => "EndCombat",
        "End of Turn" | "End Of Turn" => "End",
        "Untap" => "Untap",
        other => return unmapped(format!("key:Trigger.Phase={other}")),
    })
}

fn phase(p: &Params) -> Tr<Value> {
    keys(p, "Phase", &["Phase", "ValidPlayer"])?;
    let step = step(p.get("Phase").unwrap_or_default())?;
    let whose = match p.get("ValidPlayer") {
        None | Some("Player") => "EachPlayer",
        Some("You") => "You",
        Some("Opponent" | "Player.Opponent") => "Opponents",
        Some(other) => return unmapped(format!("key:Trigger.Phase.ValidPlayer={other}")),
    };
    Ok(json!({"StepBegins": {"step": step, "whose": whose}}))
}

fn attacks(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "Attacks", &["ValidCard", "Alone"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Creature"))?;
    let variant = if p.get("Alone") == Some("True") {
        "AttacksAlone"
    } else {
        "Attacks"
    };
    Ok(json!({ variant: {"what": what} }))
}

fn player_ref(word: Option<&str>) -> Tr<&'static str> {
    match word {
        None | Some("Player") => Ok("EachPlayer"),
        Some("You") => Ok("You"),
        Some("Opponent" | "Player.Opponent") => Ok("Opponents"),
        Some(other) => unmapped(format!("player:{other}")),
    }
}

fn spell_cast(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "SpellCast", &["ValidCard", "ValidActivatingPlayer"])?;
    let by = player_ref(p.get("ValidActivatingPlayer"))?;
    let mut pattern = json!({ "by": by });
    if let Some(valid) = p.get("ValidCard") {
        pattern["what"] = json!(selector_any(ctx, valid)?);
    }
    Ok(json!({ "SpellCast": pattern }))
}

fn damage_done(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(
        p,
        "DamageDone",
        &["ValidSource", "ValidTarget", "CombatDamage"],
    )?;
    let source = selector_any(ctx, p.get("ValidSource").unwrap_or("Card"))?;
    let mut pattern = json!({ "source": source });
    if let Some(target) = p.get("ValidTarget") {
        pattern["to"] = match target {
            "Player" => json!("EachPlayer"),
            "You" => json!("You"),
            "Opponent" | "Player.Opponent" => json!("Opponents"),
            other => return unmapped(format!("key:Trigger.DamageDone.ValidTarget={other}")),
        };
    }
    if p.get("CombatDamage") == Some("True") {
        pattern["combat"] = json!(true);
    }
    Ok(json!({ "DealsDamage": pattern }))
}

fn drawn(p: &Params) -> Tr<Value> {
    keys(p, "Drawn", &["ValidCard"])?;
    let who = match p.get("ValidCard") {
        Some("Card.YouOwn" | "Card.YouCtrl") => "You",
        Some("Card.OppOwn") => "Opponents",
        None | Some("Card") => "EachPlayer",
        Some(other) => return unmapped(format!("key:Trigger.Drawn.ValidCard={other}")),
    };
    Ok(json!({"Drew": {"who": who}}))
}

fn life_gained(p: &Params) -> Tr<Value> {
    keys(p, "LifeGained", &["ValidPlayer"])?;
    let who = player_ref(p.get("ValidPlayer"))?;
    Ok(json!({"LifeGained": {"who": who}}))
}

/// A trigger on a permanent: sacrificed, tapped, untapped, blocks, becomes blocked.
fn what_trigger(ctx: &Ctx, p: &Params, variant: &str) -> Tr<Value> {
    keys(p, variant, &["ValidCard"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Card"))?;
    Ok(json!({ variant: {"what": what} }))
}
