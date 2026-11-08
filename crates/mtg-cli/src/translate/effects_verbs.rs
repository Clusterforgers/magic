//! Effects that map one Forge verb to one engine node: keyword actions, player actions, counters, exchanges and coin flips.

use mtg_cards::effect::MOST_PASSES;
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
        "Explore" => return Some(explore(ctx, p)),
        "Incubate" => return Some(incubate(ctx, p)),
        "BecomeMonarch" => player_verb(ctx, p, api, "BecomeMonarch"),
        "TakeInitiative" => player_verb(ctx, p, api, "TakeInitiative"),
        "RingTemptsYou" => player_verb(ctx, p, api, "RingTempts"),
        "Learn" => player_verb(ctx, p, api, "Learn"),
        "TimeTravel" => player_verb(ctx, p, api, "TimeTravel"),
        "Unattach" => object_verb(ctx, p, api, "Unattach"),
        "TapOrUntap" => object_verb(ctx, p, api, "TapOrUntap"),
        "Phases" => object_verb(ctx, p, api, "PhaseOut"),
        "AddTurn" => add_turn(ctx, p),
        "Radiation" => radiation(ctx, p),
        "Blight" => return Some(blight(ctx, p)),
        "Manifest" => manifest(ctx, p, "Manifest"),
        "Cloak" => manifest(ctx, p, "Cloak"),
        "MultiplyCounter" => multiply_counter(ctx, p),
        "MoveCounter" => move_counter(ctx, p),
        "ExchangeControl" => exchange_control(ctx, p),
        "ExchangeLife" => exchange_life(ctx, p),
        "FlipCoin" => return Some(flip_coins(ctx, p)),
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

/// `Incubate`, `Times$` times over ("incubate 2 five times", "incubate 3 X times").
fn incubate(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "Incubate", &["Amount", "Defined", "Times"])?;
    let who = subject(ctx, p, "You", None)?;
    let count = amount(ctx, p.get("Amount").unwrap_or("1"))?;
    times(
        ctx,
        p.get("Times"),
        json!({"Incubate": {"who": who, "count": count}}),
    )
}

/// `Explore`, `Num$` times over ("it explores, then it explores again").
fn explore(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    let mut read = TARGETING.to_vec();
    read.push("Num");
    only(p, "Explore", &read)?;
    let what = subject(ctx, p, "Self", None)?;
    times(ctx, p.get("Num"), json!({"Explore": {"what": what}}))
}

/// The node done `count` times, one after the other: a fixed count is that many copies, another amount a loop of at most
/// `MOST_PASSES` passes that runs as many times as the amount is when it starts.
fn times(ctx: &Ctx, count: Option<&str>, node: Value) -> Tr<Vec<Value>> {
    let count = count.unwrap_or("1");
    match count.parse::<u32>() {
        Ok(n) if (1..=MOST_PASSES).contains(&n) => Ok(vec![node; n as usize]),
        Ok(_) => unmapped("key:Times=out-of-range"),
        Err(_) => Ok(vec![json!({"RepeatWhile": {
            "condition": {"Compare": {"left": 0, "op": "Eq", "right": 0}},
            "effects": [node],
            "most": MOST_PASSES,
            "times": amount(ctx, count)?,
        }})]),
    }
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

/// `Blight`: the player puts that many -1/-1 counters on a creature they control and pick.
fn blight(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "Blight", &["Num", "Defined"])?;
    if p.get("Defined").is_some_and(|who| who != "You") {
        return unmapped("key:Blight.Defined");
    }
    let count = amount(ctx, p.get("Num").unwrap_or("1"))?;
    let bind = ctx.fresh("blighted");
    Ok(vec![
        json!({"Choose": {"from": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]}, "count": {"min": 1, "max": 1}, "as": bind}}),
        json!({"PutCounters": {"on": {"Bound": bind}, "counter": "-1/-1", "count": count}}),
    ])
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
    let remember = format!("Remember{variant}ed");
    only(
        p,
        variant,
        &["Amount", "Defined", "DefinedPlayer", remember.as_str()],
    )?;
    if !matches!(p.get("Defined"), None | Some("TopOfLibrary")) {
        return unmapped(format!("key:{variant}.Defined"));
    }
    let who = defined(ctx, p.get("DefinedPlayer").unwrap_or("You"))?;
    let count = amount(ctx, p.get("Amount").unwrap_or("1"))?;
    let mut effect = json!({ variant: {"who": who, "count": count} });
    // `RememberManifested$`: the face-down permanents, as the objects they became on the battlefield.
    if p.get(&remember) == Some("True") {
        let name = ctx.fresh(&variant.to_lowercase());
        effect[variant]["as"] = json!(name);
        ctx.remembered = Some(json!({ "Bound": name }));
    }
    Ok(effect)
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

/// `MoveCounter` of one named kind, of a kind the controller picks (`CounterType$ Any`, one counter) or of every kind
/// (`CounterType$ All | CounterNum$ All`) from the `Source$` onto the target or `Defined$`.
fn move_counter(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let mut read = TARGETING.to_vec();
    read.extend(["Source", "CounterType", "CounterNum"]);
    only(p, "MoveCounter", &read)?;
    // `None`: any kind; `all`: every counter there is.
    let (kind, all) = match (p.get("CounterType"), p.get("CounterNum")) {
        (Some("Any"), None | Some("1")) => (None, false),
        (Some("All"), Some("All")) => (None, true),
        (Some(word), number) => {
            let Some(kind) = counter_kind(word) else {
                return unmapped("key:MoveCounter.Source-or-CounterType");
            };
            if number.is_some_and(|n| n.trim().parse::<u32>().is_err()) {
                return unmapped("key:MoveCounter.CounterNum");
            }
            (Some(kind), false)
        }
        (None, _) => return unmapped("key:MoveCounter.Source-or-CounterType"),
    };
    let moved = |ctx: &mut Ctx, from: Value, to: Value| -> Tr<Value> {
        let count = if all {
            json!({ "AnyCounters": from.clone() })
        } else {
            amount(ctx, p.get("CounterNum").unwrap_or("1"))?
        };
        let mut effect = json!({"from": from, "to": to, "count": count});
        if let Some(kind) = &kind {
            effect["counter"] = json!(kind);
        }
        Ok(json!({ "MoveCounters": effect }))
    };
    // "Move a counter from target creature onto a second target creature": two targets of the same kind.
    let two = p.get("TargetMin") == Some("2") && p.get("TargetMax") == Some("2");
    if two && p.has("ValidTgts") && !p.has("Source") && !p.has("Defined") {
        let one = p.without(&["TargetMin", "TargetMax"]);
        let (from, to) = (
            subject(ctx, &one, "Self", None)?,
            subject(ctx, &one, "Self", None)?,
        );
        return moved(ctx, from, to);
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
    moved(ctx, from, to)
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
            "FlipUntilYouLose",
            "Defined",
            "ValidTgts",
            "Amount",
            "RememberNumber",
        ],
    )?;
    // `RememberNumber$ Wins` keeps the wins for a part that reads `Wins`; the win part runs once per win, where it is one.
    if p.get("RememberNumber").is_some_and(|word| word != "Wins") {
        return unmapped("key:FlipCoin.RememberNumber");
    }
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
    match p.get("FlipUntilYouLose") {
        None => {}
        Some("True") => node["FlipCoin"]["until"] = json!(true),
        Some(_) => return unmapped("key:FlipCoin.FlipUntilYouLose"),
    }
    for (key, field) in [("WinSubAbility", "win"), ("LoseSubAbility", "lose")] {
        if let Some(name) = p.get(key) {
            let part = ctx.sub(name)?;
            ctx.flip_win = key == "WinSubAbility";
            let chain = chain_at(ctx, &part, false);
            ctx.flip_win = false;
            node["FlipCoin"][field] = Value::Array(chain?);
        }
    }
    Ok(node)
}

/// `FlipCoin` with `Amount$ N`: N flips in a row, each with the win part ("for each flip you win"), so a part that reads
/// the wins reads one. A lose part with several flips is a gap.
fn flip_coins(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    let times = match p.get("Amount") {
        None => 1,
        Some(word) => match word.parse::<usize>() {
            Ok(n) if (1..=10).contains(&n) => n,
            _ => return unmapped("key:FlipCoin.Amount"),
        },
    };
    if times > 1 && (p.has("LoseSubAbility") || p.get("FlipUntilYouLose").is_some()) {
        return unmapped("key:FlipCoin.Amount-with-more");
    }
    let node = flip_coin(ctx, p)?;
    Ok(vec![node; times])
}

/// `Clash` of the controller with an opponent they choose: the win and the otherwise part.
fn clash(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "Clash",
        &["WinSubAbility", "OtherwiseSubAbility", "ValidTgts"],
    )?;
    // "If you win, target creature gains double strike": the target is chosen as the ability is put on the stack, and
    // the parts name it.
    if p.has("ValidTgts") {
        super::valid::subject(ctx, p, "", None)?;
    }
    let mut node = json!({"Clash": {}});
    for (key, field) in [("WinSubAbility", "win"), ("OtherwiseSubAbility", "lose")] {
        if let Some(name) = p.get(key) {
            let part = ctx.sub(name)?;
            node["Clash"][field] = Value::Array(chain_at(ctx, &part, false)?);
        }
    }
    Ok(node)
}
