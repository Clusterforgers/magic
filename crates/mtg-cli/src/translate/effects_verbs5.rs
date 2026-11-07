//! Effects on players and the game that map onto existing nodes: poison counters, the fog shield, skipped steps,
//! sacrificing named objects, earthbend and discover.

use mtg_cards::effect::{Effect, discover};
use mtg_cards::macros;
use mtg_cards::refs::Ref;
use mtg_cards::value::Amount;
use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, only, unmapped};
use super::valid::{defined, selector_any, subject};
use crate::forge::script::Params;

/// The effects of a verb this module reads; `None` for any other API.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Option<Tr<Vec<Value>>> {
    Some(match api {
        "Poison" => poison(ctx, p).map(|node| vec![node]),
        "Fog" => fog(p).map(|node| vec![node]),
        "MustBlock" => must_block(ctx, p).map(|node| vec![node]),
        "SkipPhase" => skip_phase(ctx, p).map(|node| vec![node]),
        "SacrificeAll" => sacrifice_all(ctx, p).map(|node| vec![node]),
        "Earthbend" => earthbend(ctx, p),
        "Discover" => discover_cards(ctx, p),
        _ => return None,
    })
}

/// `Poison`: poison counters on the player the line names.
fn poison(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "Poison",
        &["Num", "Defined", "ValidTgts", "TargetMin", "TargetMax"],
    )?;
    let Some(num) = p.get("Num") else {
        return unmapped("key:Poison.no-Num");
    };
    let count = amount(ctx, num)?;
    let on = subject(ctx, p, "You", None)?;
    Ok(json!({"PutCounters": {"on": on, "counter": "poison", "count": count}}))
}

/// `Fog`: all combat damage this turn is prevented.
fn fog(p: &Params) -> Tr<Value> {
    only(p, "Fog", &[])?;
    Ok(json!({"Shield": {
        "when": {"Damage": {"combat": true}}, "by": "Prevent", "duration": "UntilEndOfTurn"}}))
}

/// `SkipPhase` of the draw or untap step for the rest of this turn. "Their next" step has no duration of its own.
fn skip_phase(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "SkipPhase",
        &["Step", "Duration", "Defined", "ValidTgts", "IsCurse"],
    )?;
    let step = match p.get("Step") {
        Some("Draw") => "Draw",
        Some("Untap") => "Untap",
        _ => return unmapped("key:SkipPhase.Step"),
    };
    if p.get("Duration") != Some("EndOfTurn") {
        return unmapped("key:SkipPhase.next-occurrence");
    }
    let who = subject(ctx, p, "You", None)?;
    Ok(json!({"Rule": {
        "rule": {"Cant": {"who": who, "action": {"SkipStep": step}}}, "duration": "UntilEndOfTurn"}}))
}

/// `SacrificeAll` of named objects: their controller sacrifices each one. A valid-string of permanents stays a gap.
fn sacrifice_all(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "SacrificeAll",
        &["Defined", "Controller", "ValidCards", "RememberSacrificed"],
    )?;
    let mut node = match p.get("Defined") {
        None => sacrifice_group(ctx, p)?,
        Some(word) => sacrifice_named(ctx, p, word)?,
    };
    if super::remember::wanted(p, &["RememberSacrificed"]) {
        super::remember::bind_moved(ctx, &mut node, "Sacrifice", "sacrificed");
    }
    Ok(node)
}

/// `SacrificeAll` of the objects `word` names.
fn sacrifice_named(ctx: &mut Ctx, p: &Params, word: &str) -> Tr<Value> {
    let object = defined(ctx, word)?;
    let who = match p.get("Controller") {
        None => json!({"ControllerOf": object}),
        Some("You") => json!("You"),
        Some(_) => return unmapped("key:SacrificeAll.Controller"),
    };
    Ok(json!({"Sacrifice": {"who": who, "filter": [{"Is": object}]}}))
}

/// `SacrificeAll` of a valid-string: each player the filter names (the controller it words, else every player)
/// sacrifices all the permanents they control that match, as a count the filter itself gives, which no hand of one
/// player exceeds.
fn sacrifice_group(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    if p.has("Controller") {
        return unmapped("key:SacrificeAll.Controller");
    }
    let Some(valid) = p.get("ValidCards") else {
        return unmapped("key:SacrificeAll.no-ValidCards");
    };
    let filter = selector_any(ctx, valid)?;
    let controlled = |who: &str| filter.contains(&json!({"ControlledBy": who}));
    let who = if controlled("You") {
        json!("You")
    } else if controlled("Opponents") {
        json!("Opponents")
    } else {
        json!("EachPlayer")
    };
    let mut effect = json!({"filter": filter, "count": {"Count": filter}});
    if who != json!("You") {
        effect["who"] = who;
    }
    Ok(json!({ "Sacrifice": effect }))
}

/// `Earthbend`: a land its controller has becomes a creature with counters, through the keyword action's table entry.
fn earthbend(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "Earthbend", &["Num"])?;
    let count = amount_of(ctx, p, "Earthbend")?;
    let on =
        ctx.target(json!({"what": {"Object": [{"Type": "Land"}, {"ControlledBy": "You"}]}}))?;
    let on: Ref = from_value(on, "Earthbend")?;
    let Some(effects) = macros::action_effects_of("earthbend", &count, &on) else {
        return unmapped("effect:Earthbend");
    };
    to_values(&effects)
}

/// `Discover`: the steps of discover N.
fn discover_cards(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "Discover", &["Num"])?;
    let count = amount_of(ctx, p, "Discover")?;
    to_values(&discover(count))
}

fn amount_of(ctx: &Ctx, p: &Params, api: &str) -> Tr<Amount> {
    let Some(num) = p.get("Num") else {
        return unmapped(format!("key:{api}.no-Num"));
    };
    from_value(amount(ctx, num)?, api)
}

fn from_value<T: serde::de::DeserializeOwned>(value: Value, api: &str) -> Tr<T> {
    serde_json::from_value(value).or_else(|_| unmapped(format!("effect:{api}-value")))
}

fn to_values(effects: &[Effect]) -> Tr<Vec<Value>> {
    effects
        .iter()
        .map(|effect| serde_json::to_value(effect).or_else(|_| unmapped("effect:serialize")))
        .collect()
}

/// `MustBlock` on a target: the creature blocks the source creature this turn if able (the requirement of Provoke).
fn must_block(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "MustBlock",
        &[
            "ValidTgts",
            "Duration",
            "DefinedAttacker",
            "TargetMin",
            "TargetMax",
        ],
    )?;
    if !p.has("ValidTgts") {
        return unmapped("key:MustBlock.no-target");
    }
    // Without `DefinedAttacker$` the attacker is the source itself, whatever the trigger.
    let attacker = match p.get("DefinedAttacker") {
        None => json!("This"),
        Some("ParentTarget") => ctx.parent()?,
        Some("TriggeredAttacker" | "TriggeredAttackerLKICopy")
            if ctx.trigger.as_ref().is_some_and(|t| {
                matches!(
                    t.mode.as_str(),
                    "Attacks" | "AttackerBlocked" | "AttackerUnblocked"
                )
            }) =>
        {
            json!("Triggering")
        }
        Some(other) => return unmapped(format!("key:MustBlock.DefinedAttacker={other}")),
    };
    let duration = super::effects_obj::duration(p.get("Duration"))?;
    let who = if attacker.get("Target").is_some() {
        // A creature can't block itself: the blocker is another target ("another target creature blocks it").
        let mut spec = super::valid::target_spec(ctx, p, None)?;
        let other = json!({"Not": {"Is": attacker}});
        match spec
            .pointer_mut("/what/Object")
            .and_then(Value::as_array_mut)
        {
            Some(preds) if !preds.contains(&other) => preds.push(other),
            Some(_) => {}
            None => return unmapped("key:MustBlock.target-of-objects"),
        }
        ctx.target(spec)?
    } else {
        subject(ctx, p, "Self", None)?
    };
    Ok(json!({"Rule": {
        "rule": {"MustBlockAttacker": {"who": who, "attacker": attacker}}, "duration": duration}}))
}
