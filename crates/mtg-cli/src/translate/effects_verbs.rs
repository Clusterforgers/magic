//! Effects that map one Forge verb to one engine node: keyword actions, player actions, counters, exchanges and coin flips.

use serde_json::{Value, json};

use super::amount::{amount, counter_kind};
use super::ctx::{Ctx, Tr, only, unmapped};
use super::effects::chain_at;
use super::valid::{defined, subject};
use crate::forge::script::Params;

const TARGETING: [&str; 4] = ["Defined", "ValidTgts", "TargetMin", "TargetMax"];

/// The effects of a verb this module reads; `None` for any other API.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Option<Tr<Vec<Value>>> {
    let node = match api {
        "Explore" => object_verb(ctx, p, api, "Explore"),
        "BecomeMonarch" => player_verb(ctx, p, api, "BecomeMonarch"),
        "TakeInitiative" => player_verb(ctx, p, api, "TakeInitiative"),
        "RingTemptsYou" => player_verb(ctx, p, api, "RingTempts"),
        "Learn" => player_verb(ctx, p, api, "Learn"),
        "TimeTravel" => player_verb(ctx, p, api, "TimeTravel"),
        "Incubate" => incubate(ctx, p),
        "Unattach" => object_verb(ctx, p, api, "Unattach"),
        "TapOrUntap" => object_verb(ctx, p, api, "TapOrUntap"),
        "Phases" => object_verb(ctx, p, api, "PhaseOut"),
        "AddTurn" => add_turn(ctx, p),
        "Radiation" => radiation(ctx, p),
        "Manifest" => manifest(ctx, p, "Manifest"),
        "Cloak" => manifest(ctx, p, "Cloak"),
        "MultiplyCounter" => multiply_counter(ctx, p),
        "MoveCounter" => move_counter(ctx, p),
        "ExchangeControl" => exchange_control(ctx, p),
        "ExchangeLife" => exchange_life(ctx, p),
        "FlipCoin" => flip_coin(ctx, p),
        "Clash" => clash(ctx, p),
        _ => return None,
    };
    Some(node.map(|node| vec![node]))
}

/// A verb whose only object is a player: the target or `Defined$`, `You` by default.
fn player_verb(ctx: &mut Ctx, p: &Params, api: &str, variant: &str) -> Tr<Value> {
    only(p, api, &TARGETING)?;
    let who = subject(ctx, p, "You", None)?;
    Ok(json!({ variant: {"who": who} }))
}

/// A verb whose only object is a permanent: the target or `Defined$`, the card itself by default.
fn object_verb(ctx: &mut Ctx, p: &Params, api: &str, variant: &str) -> Tr<Value> {
    only(p, api, &TARGETING)?;
    let what = subject(ctx, p, "Self", None)?;
    Ok(json!({ variant: {"what": what} }))
}

fn incubate(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(p, "Incubate", &["Amount", "Defined"])?;
    let who = subject(ctx, p, "You", None)?;
    let count = amount(ctx, p.get("Amount").unwrap_or("1"))?;
    Ok(json!({"Incubate": {"who": who, "count": count}}))
}

/// `SkipTurn`: each turn is one rule that the player skips a turn and ends with the turn it skips (CR 614.10).
pub fn skip_turn(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    let mut read = TARGETING.to_vec();
    read.push("NumTurns");
    only(p, "SkipTurn", &read)?;
    let who = subject(ctx, p, "You", None)?;
    let Ok(turns) = p.get("NumTurns").unwrap_or("1").parse::<usize>() else {
        return unmapped("key:SkipTurn.NumTurns");
    };
    let rule = json!({"Rule": {"rule": {"Cant": {"who": who, "action": "SkipTurn"}}, "duration": "UntilTurnSkipped"}});
    Ok(vec![rule; turns])
}

/// `Radiation`: the players get rad counters (CR 725), counters of the kind `rad` on a player.
fn radiation(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let mut read = TARGETING.to_vec();
    read.push("Num");
    only(p, "Radiation", &read)?;
    let who = subject(ctx, p, "You", None)?;
    let count = amount(ctx, p.get("Num").unwrap_or("1"))?;
    Ok(json!({"PutCounters": {"on": who, "counter": {"Other": "rad"}, "count": count}}))
}

fn add_turn(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let mut read = TARGETING.to_vec();
    read.push("NumTurns");
    only(p, "AddTurn", &read)?;
    let who = subject(ctx, p, "You", None)?;
    let count = amount(ctx, p.get("NumTurns").unwrap_or("1"))?;
    Ok(json!({"ExtraTurn": {"who": who, "count": count}}))
}

/// `Manifest` and `Cloak` of the top cards of a library; a named card (`Remembered`) is a gap.
fn manifest(ctx: &mut Ctx, p: &Params, variant: &str) -> Tr<Value> {
    only(p, variant, &["Amount", "Defined", "DefinedPlayer"])?;
    if !matches!(p.get("Defined"), None | Some("TopOfLibrary")) {
        return unmapped(format!("key:{variant}.Defined"));
    }
    let who = defined(ctx, p.get("DefinedPlayer").unwrap_or("You"))?;
    let count = amount(ctx, p.get("Amount").unwrap_or("1"))?;
    Ok(json!({ variant: {"who": who, "count": count} }))
}

fn multiply_counter(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let mut read = TARGETING.to_vec();
    read.extend(["CounterType", "Multiplier"]);
    only(p, "MultiplyCounter", &read)?;
    // No kind means each kind the object holds ("double the number of each kind of counter on it").
    let kind = match p.get("CounterType") {
        None => Value::Null,
        Some(word) => match counter_kind(word) {
            Some(kind) => kind,
            None => return unmapped("key:MultiplyCounter.CounterType"),
        },
    };
    let factor = match p.get("Multiplier") {
        None => 2,
        Some(text) => match text.trim().parse::<u32>() {
            Ok(n) if n > 1 => n,
            _ => return unmapped("key:MultiplyCounter.Multiplier"),
        },
    };
    let on = subject(ctx, p, "Self", None)?;
    let mut effect = json!({"MultiplyCounters": {"on": on, "factor": factor}});
    if !kind.is_null() {
        effect["MultiplyCounters"]["counter"] = kind;
    }
    Ok(effect)
}

/// `MoveCounter` of one named kind from the `Source$` onto the target or `Defined$`.
fn move_counter(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let mut read = TARGETING.to_vec();
    read.extend(["Source", "CounterType", "CounterNum"]);
    only(p, "MoveCounter", &read)?;
    let Some(kind) = p.get("CounterType").and_then(counter_kind) else {
        return unmapped("key:MoveCounter.Source-or-CounterType");
    };
    if p.get("CounterNum")
        .is_some_and(|n| n.trim().parse::<u32>().is_err())
    {
        return unmapped("key:MoveCounter.CounterNum");
    }
    // Without `Source$` the target is the source and `Defined$` the object that receives them.
    let (from, to) = match (p.get("Source"), p.get("Defined"), p.has("ValidTgts")) {
        (Some(source), _, _) => (defined(ctx, source)?, subject(ctx, p, "Self", None)?),
        (None, Some(receiver), true) => (
            subject(ctx, &p.without(&["Defined"]), "Self", None)?,
            defined(ctx, receiver)?,
        ),
        _ => return unmapped("key:MoveCounter.Source-or-CounterType"),
    };
    let count = amount(ctx, p.get("CounterNum").unwrap_or("1"))?;
    Ok(json!({"MoveCounters": {"from": from, "to": to, "counter": kind, "count": count}}))
}

/// `ExchangeControl` for good between the `Defined$` permanent and the one target.
fn exchange_control(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(p, "ExchangeControl", &TARGETING)?;
    if !p.has("Defined") || !p.has("ValidTgts") || p.has("TargetMin") || p.has("TargetMax") {
        return unmapped("key:ExchangeControl.shape");
    }
    let first = defined(ctx, p.get("Defined").unwrap_or_default())?;
    let second = subject(ctx, &p.without(&["Defined"]), "Self", None)?;
    Ok(json!({"ExchangeControl": {"what": [first, second], "duration": "Forever"}}))
}

/// `ExchangeLife` with one target player.
fn exchange_life(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(p, "ExchangeLife", &["ValidTgts"])?;
    if !p.has("ValidTgts") {
        return unmapped("key:ExchangeLife.no-target");
    }
    let with = subject(ctx, p, "You", None)?;
    Ok(json!({"ExchangeLife": {"with": with}}))
}

/// `FlipCoin` of the controller, once: the named win and lose parts.
fn flip_coin(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "FlipCoin",
        &[
            "WinSubAbility",
            "LoseSubAbility",
            "NoCall",
            "Defined",
            "ValidTgts",
        ],
    )?;
    if !matches!(p.get("Defined"), None | Some("You")) {
        return unmapped("key:FlipCoin.Defined");
    }
    // The flip names a target that the win and lose parts read as `Targeted`.
    if p.has("ValidTgts") {
        subject(ctx, p, "You", None)?;
    }
    // With or without a call the flip is won with even odds, as the node says.
    let _ = p.get("NoCall");
    let mut node = json!({"FlipCoin": {}});
    for (key, field) in [("WinSubAbility", "win"), ("LoseSubAbility", "lose")] {
        if let Some(name) = p.get(key) {
            let part = ctx.sub(name)?;
            node["FlipCoin"][field] = Value::Array(chain_at(ctx, &part, false)?);
        }
    }
    Ok(node)
}

/// `Clash` of the controller with an opponent they choose: the win and the otherwise part.
fn clash(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(p, "Clash", &["WinSubAbility", "OtherwiseSubAbility"])?;
    let mut node = json!({"Clash": {}});
    for (key, field) in [("WinSubAbility", "win"), ("OtherwiseSubAbility", "lose")] {
        if let Some(name) = p.get(key) {
            let part = ctx.sub(name)?;
            node["Clash"][field] = Value::Array(chain_at(ctx, &part, false)?);
        }
    }
    Ok(node)
}
