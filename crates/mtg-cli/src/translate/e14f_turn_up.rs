//! `Event$ TurnFaceUp` replacement lines: "As ~ is turned face up, put N +1/+1 counters on it" (CR 708.8), and the
//! rules that a permanent can't be turned face up (`Layer$ CantHappen`).

use serde_json::{Value, json};

use super::amount::{amount, counter_kind};
use super::ctx::{Ctx, Tr, check_keys, unmapped};
use crate::forge::script::Params;

/// A `TurnFaceUp` line: counters the permanent itself gets as it turns, or a group that can't be turned face up. Any
/// other shape stays a gap.
pub fn turn_up(ctx: &mut Ctx, p: &Params, condition: Option<&Value>) -> Tr<Value> {
    if condition.is_some() {
        return unmapped("key:Replacement.TurnFaceUp.condition");
    }
    if !matches!(p.get("ActiveZones"), None | Some("Battlefield")) {
        return unmapped("replacement:TurnFaceUp-zone");
    }
    if p.get("Layer") == Some("CantHappen") {
        return cant(p);
    }
    check_keys(
        p,
        "Replacement.TurnFaceUp",
        &[
            "Event",
            "Description",
            "ValidCard",
            "ReplaceWith",
            "ActiveZones",
            "Secondary",
        ],
        |_, _| false,
    )?;
    if p.get("ValidCard") != Some("Card.Self") {
        return unmapped("replacement:TurnFaceUp-shape");
    }
    let part = ctx.sub(p.get("ReplaceWith").unwrap_or_default())?;
    if part.get("DB") != Some("PutCounter") || part.get("Defined") != Some("Self") {
        return unmapped("replacement:TurnFaceUp-shape");
    }
    check_keys(
        &part,
        "Replacement.TurnFaceUp",
        &["DB", "Defined", "CounterNum", "CounterType"],
        |_, _| false,
    )?;
    let Some(counter) = counter_kind(part.get("CounterType").unwrap_or_default()) else {
        return unmapped("replacement:TurnFaceUp-counter");
    };
    let count = amount(ctx, part.get("CounterNum").unwrap_or("1"))?;
    Ok(json!({"Replacement": {
        "when": {"TurnedFaceUp": {"what": ["This"]}},
        "by": {"WithCounters": {"counter": counter, "count": count}},
    }}))
}

/// "Permanents your opponents control can't be turned face up during your turn", "As long as enchanted creature is
/// face down, it can't be turned face up" (a face-up creature can't be turned face up anyway).
fn cant(p: &Params) -> Tr<Value> {
    check_keys(
        p,
        "Replacement.TurnFaceUp",
        &[
            "Event",
            "Description",
            "ValidCard",
            "Layer",
            "ActiveZones",
            "PlayerTurn",
        ],
        |_, _| false,
    )?;
    let affected = match p.get("ValidCard") {
        Some("Permanent.OppCtrl") => json!({"Each": [{"ControlledBy": "Opponents"}]}),
        Some("Card.EnchantedBy+faceDown") => json!("Attached"),
        _ => return unmapped("replacement:TurnFaceUp-cant-shape"),
    };
    let mut ability = json!({"affected": affected, "mods": ["CantBeTurnedFaceUp"]});
    match p.get("PlayerTurn") {
        None => {}
        Some("True") => ability["condition"] = json!("YourTurn"),
        Some(_) => return unmapped("key:Replacement.TurnFaceUp.PlayerTurn"),
    }
    Ok(json!({ "Static": ability }))
}
