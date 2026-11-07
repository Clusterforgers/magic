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
        "Shuffle" => player_verb(ctx, p, api, "Shuffle"),
        "Venture" => venture(p),
        "EndTurn" => end_turn(p),
        "Amass" => amass(ctx, p),
        "Play" => return Some(super::mayplay::play(ctx, p)),
        "Connive" => object_verb(ctx, p, api, "Connive"),
        "Goad" => object_verb(ctx, p, api, "Goad"),
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

/// A keyword action on an object: `what` is the target or `Defined$`, the card itself by default.
fn object_verb(ctx: &mut Ctx, p: &Params, api: &str, variant: &str) -> Tr<Value> {
    only(p, api, &TARGETING)?;
    let what = subject(ctx, p, "Self", None)?;
    Ok(json!({ variant: {"what": what} }))
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
    if super::remember::wanted(p, &["RememberAmass"]) {
        super::remember::bind_moved(ctx, &mut node, "Amass", "amassed");
    }
    Ok(node)
}
