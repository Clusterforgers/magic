//! Replacements that a permanent holds beside the entering ones: skipped steps (`Event$ BeginPhase`) and the life floor
//! (`Event$ LifeReduced`).

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use crate::forge::script::Params;

/// `Event$ BeginPhase | Skip$ True`: the players skip the step (CR 614.10) as a rule of the permanent.
pub fn skip_step(p: &Params, line_condition: Option<&Value>) -> Tr<Value> {
    if line_condition.is_some() {
        return unmapped("key:Replacement.BeginPhase.condition");
    }
    check_keys(
        p,
        "Replacement.BeginPhase",
        &["Event", "ActiveZones", "ValidPlayer", "Phase", "Skip"],
        |_, _| false,
    )?;
    if p.get("Skip") != Some("True") || p.get("ActiveZones") != Some("Battlefield") {
        return unmapped("replacement:BeginPhase");
    }
    let step = match p.get("Phase") {
        Some("Upkeep") => "Upkeep",
        Some("Draw") => "Draw",
        Some("Untap") => "Untap",
        _ => return unmapped("key:Replacement.BeginPhase.Phase"),
    };
    let who = match p.get("ValidPlayer") {
        None | Some("Player") => "EachPlayer",
        Some("You") => "You",
        Some("Opponent") => "Opponents",
        Some(_) => return unmapped("key:Replacement.BeginPhase.ValidPlayer"),
    };
    Ok(json!({"Rule": {"rule": {"Cant": {"who": who, "action": {"SkipStep": step}}}}}))
}

/// `Event$ LifeReduced | IsDamage$ True`: "damage that would reduce your life total to less than N reduces it to N
/// instead" as the life floor of a permanent, from `You.lifeGEn`, `Result$ LTn` and the `ReplaceEffect` that keeps `n`.
pub fn life_floor(ctx: &Ctx, p: &Params, line_condition: Option<&Value>) -> Tr<Value> {
    check_keys(
        p,
        "Replacement.LifeReduced",
        &[
            "Event",
            "ActiveZones",
            "ValidPlayer",
            "Result",
            "IsDamage",
            "ReplaceWith",
            "IsPresent",
        ],
        |_, _| false,
    )?;
    let floor = p
        .get("ValidPlayer")
        .and_then(|word| word.strip_prefix("You.lifeGE"))
        .and_then(|n| n.parse::<u32>().ok());
    let Some(floor) = floor.filter(|floor| {
        p.get("Result") == Some(format!("LT{floor}").as_str())
            && p.get("IsDamage") == Some("True")
            && p.get("ActiveZones") == Some("Battlefield")
            && keeps_floor(ctx, p.get("ReplaceWith"), *floor)
    }) else {
        return unmapped("replacement:LifeReduced");
    };
    let mut ability = json!({"when": {"Damage": {"to": "You"}}, "by": {"LifeFloor": floor}});
    if line_condition.is_some() {
        return unmapped("key:Replacement.LifeReduced.condition");
    }
    if let Some(condition) = super::condition::condition(ctx, p)? {
        ability["condition"] = condition;
    }
    Ok(json!({ "Replacement": ability }))
}

/// Whether the `ReplaceWith$` part limits the lost amount to the life above `floor` and changes nothing else.
fn keeps_floor(ctx: &Ctx, part: Option<&str>, floor: u32) -> bool {
    let Some(part) = part.and_then(|name| ctx.svar(name)).map(Params::parse) else {
        return false;
    };
    let limit = part.get("VarValue").and_then(|name| ctx.svar(name));
    let above = ctx.svar("Difference");
    part.get("DB") == Some("ReplaceEffect")
        && part.get("VarName") == Some("Amount")
        && limit == Some("ReplaceCount$Amount/LimitMax.Difference")
        && above == Some(format!("Count$YourLifeTotal/Minus.{floor}").as_str())
}
