//! Effects on the game and its players: winning and losing, life totals, ending the turn, and the keyword actions
//! that name only an object (goad, connive, amass, venture).

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, only};
use super::valid::subject;
use crate::forge::script::Params;

/// The effects of a verb this module reads; `None` for any other API.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Option<Tr<Vec<Value>>> {
    let node = match api {
        "SetLife" => set_life(ctx, p),
        "LosesGame" => player_verb(ctx, p, api, "LoseGame"),
        "WinsGame" => player_verb(ctx, p, api, "WinGame"),
        "Shuffle" => return Some(shuffle(ctx, p)),
        "Venture" => venture(p),
        "OpenAttraction" => open_attraction(p),
        "PutSticker" => put_sticker(ctx, p),
        "EndTurn" => end_turn(p),
        "Amass" => amass(ctx, p),
        "Play" => return Some(super::mayplay::play(ctx, p)),
        "Connive" => connive(ctx, p),
        "Goad" => goad(ctx, p),
        "RemoveFromCombat" => object_verb(ctx, p, api, "RemoveFromCombat"),
        _ => return None,
    };
    Some(node.map(|node| vec![node]))
}

const TARGETING: [&str; 4] = ["Defined", "ValidTgts", "TargetMin", "TargetMax"];

/// A verb whose only object is a player: `who` is the target or `Defined$`, `You` by default.
fn player_verb(ctx: &mut Ctx, p: &Params, api: &str, variant: &str) -> Tr<Value> {
    only(p, api, &TARGETING)?;
    let who = subject(ctx, p, "You", None)?;
    Ok(json!({ variant: {"who": who} }))
}

/// `Shuffle`: the player's library is shuffled, if the player the line names decides to when it is optional ("you may then
/// have that player shuffle that library").
fn shuffle(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    let mut read = TARGETING.to_vec();
    read.push("Optional");
    only(p, "Shuffle", &read)?;
    let who = subject(ctx, p, "You", None)?;
    super::frame::optional(
        p,
        "Shuffle",
        vec![json!({"Shuffle": {"who": who}})],
        &json!("You"),
    )
}

/// A keyword action on an object: `what` is the target or `Defined$`, the card itself by default.
fn object_verb(ctx: &mut Ctx, p: &Params, api: &str, variant: &str) -> Tr<Value> {
    only(p, api, &TARGETING)?;
    let what = subject(ctx, p, "Self", None)?;
    Ok(json!({ variant: {"what": what} }))
}

/// `Connive`: the creatures connive, `ConniveNum$` times over ("connives X", CR 701.50e).
fn connive(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let mut read = TARGETING.to_vec();
    read.push("ConniveNum");
    only(p, "Connive", &read)?;
    let what = subject(ctx, p, "Self", None)?;
    let mut node = json!({"Connive": {"what": what}});
    if let Some(count) = p.get("ConniveNum") {
        node["Connive"]["count"] = amount(ctx, count)?;
    }
    Ok(node)
}

/// `Goad`: until the goader's next turn (CR 701.15); with `Duration$` ("goaded for the rest of the game", "for as long as
/// ~ remains on the battlefield") the same continuous `Goaded` for that long.
fn goad(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let mut read = TARGETING.to_vec();
    read.push("Duration");
    only(p, "Goad", &read)?;
    let what = subject(ctx, p, "Self", None)?;
    let duration = match p.get("Duration") {
        None => return Ok(json!({"Goad": {"what": what}})),
        Some("Permanent") => "Forever",
        Some("AsLongAsInPlay") => "WhileSourceOnBattlefield",
        Some(other) => return super::ctx::unmapped(format!("key:Goad.Duration={other}")),
    };
    Ok(json!({"Continuous": {"affected": what, "mods": ["Goaded"], "duration": duration}}))
}

fn set_life(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let mut read = TARGETING.to_vec();
    read.push("LifeAmount");
    only(p, "SetLife", &read)?;
    let who = subject(ctx, p, "You", None)?;
    let amount = amount(ctx, p.get("LifeAmount").unwrap_or("0"))?;
    Ok(json!({"SetLife": {"who": who, "amount": amount}}))
}

fn venture(p: &Params) -> Tr<Value> {
    only(p, "Venture", &["Defined"])?;
    match p.get("Defined") {
        None | Some("You") => Ok(json!({"Venture": {}})),
        Some(_) => super::ctx::unmapped("key:Venture.Defined"),
    }
}

/// `PutSticker` (CR 123): the player puts a sticker on a permanent of `ValidCard$` they own; `Optional$ True` is
/// "you may".
fn put_sticker(ctx: &Ctx, p: &Params) -> Tr<Value> {
    only(p, "PutSticker", &["Defined", "ValidCard", "Optional"])?;
    if !matches!(p.get("Defined"), None | Some("You")) {
        return super::ctx::unmapped("key:PutSticker.Defined");
    }
    let Some(valid) = p.get("ValidCard") else {
        return super::ctx::unmapped("key:PutSticker.ValidCard");
    };
    let on = super::valid::selector(ctx, valid)?;
    // A sticker goes only on a permanent its player owns.
    if !on.contains(&json!({"OwnedBy": "You"})) {
        return super::ctx::unmapped("key:PutSticker.ValidCard-owner");
    }
    let effect = json!({"PutSticker": {"on": on}});
    match p.get("Optional") {
        None | Some("False") => Ok(effect),
        Some("True") => Ok(json!({"May": {"effects": [effect]}})),
        Some(_) => super::ctx::unmapped("key:PutSticker.Optional"),
    }
}

/// `OpenAttraction` (CR 701.51): the player opens `Amount$` Attractions, one by default.
fn open_attraction(p: &Params) -> Tr<Value> {
    only(p, "OpenAttraction", &["Defined", "Amount"])?;
    if !matches!(p.get("Defined"), None | Some("You")) {
        return super::ctx::unmapped("key:OpenAttraction.Defined");
    }
    match p.get("Amount").map(str::parse::<u32>) {
        None | Some(Ok(1)) => Ok(json!({"OpenAttraction": {}})),
        Some(Ok(count)) if count > 1 => Ok(json!({"OpenAttraction": {"count": count}})),
        Some(_) => super::ctx::unmapped("key:OpenAttraction.Amount"),
    }
}

fn end_turn(p: &Params) -> Tr<Value> {
    only(p, "EndTurn", &[])?;
    Ok(json!("EndTurn"))
}

fn amass(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(p, "Amass", &["Num", "Type", "RememberAmass"])?;
    let Some(kind) = p.get("Type") else {
        return super::ctx::unmapped("key:Amass.no-Type");
    };
    let count = amount(ctx, p.get("Num").unwrap_or("1"))?;
    let mut node = json!({"Amass": {"subtype": kind, "count": count}});
    // "The amassed Army": the Army that got the counters.
    if super::remember::wanted(p, "Amass") {
        super::remember::bind_moved(ctx, &mut node, "Amass", "amassed");
    }
    Ok(node)
}
