//! `Event$ LifeReduced` replacements: damage that would take a life total below a floor leaves it at the floor, and
//! damage that causes no loss of life.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use crate::forge::script::Params;

/// A `LifeReduced` line about damage to you as a `Rule` ability that holds while the line's condition does.
pub fn life_reduced(ctx: &Ctx, p: &Params, condition: Option<&Value>) -> Tr<Value> {
    // "If you control a creature" (`IsPresent$`) and "as long as you're the monarch" (`Monarch$ True`) are the condition of the rule.
    let mut held: Vec<Value> = condition.cloned().into_iter().collect();
    held.extend(super::condition::condition(ctx, p)?);
    let p = &super::condition::strip(p);
    let p = &match p.get("Monarch") {
        Some("True") => {
            held.push(json!({"Monarch": "You"}));
            p.without(&["Monarch"])
        }
        _ => p.clone(),
    };
    let rule = if p.get("Layer") == Some("CantHappen") {
        check_keys(
            p,
            "Replacement.LifeReduced",
            &["Event", "ValidPlayer", "IsDamage", "Layer"],
            |key, value| key == "ActiveZones" && value == "Battlefield",
        )?;
        if p.get("ValidPlayer") != Some("You") || p.get("IsDamage") != Some("True") {
            return unmapped("key:Replacement.LifeReduced.shape");
        }
        json!({"NoLifeLossFromDamage": "You"})
    } else {
        floor(ctx, p)?
    };
    let mut ability = json!({"Rule": {"rule": rule}});
    match held.len() {
        0 => {}
        1 => ability["Rule"]["condition"] = held.remove(0),
        _ => ability["Rule"]["condition"] = json!({"All": held}),
    }
    Ok(ability)
}

/// "Damage that would reduce your life total to less than N reduces it to N instead", while the life total is N or more.
fn floor(ctx: &Ctx, p: &Params) -> Tr<Value> {
    check_keys(
        p,
        "Replacement.LifeReduced",
        &["Event", "ValidPlayer", "IsDamage", "Result", "ReplaceWith"],
        |key, value| key == "ActiveZones" && value == "Battlefield",
    )?;
    let (Some(player), Some(result), Some("True")) =
        (p.get("ValidPlayer"), p.get("Result"), p.get("IsDamage"))
    else {
        return unmapped("key:Replacement.LifeReduced.shape");
    };
    let floor = result
        .strip_prefix("LT")
        .and_then(|n| n.parse::<i32>().ok())
        .filter(|floor| player == format!("You.lifeGE{floor}"));
    // The replacement must be the one that takes the life lost down to the difference from the floor.
    let reduces = p.get("ReplaceWith").is_some_and(|name| {
        ctx.svar(name)
            .is_some_and(|text| text.contains("ReplaceEffect"))
            && ctx
                .svar("X")
                .is_some_and(|x| x.contains("ReplaceCount$Amount/LimitMax.Difference"))
    });
    match floor {
        Some(floor) if reduces => Ok(json!({"LifeFloor": {"who": "You", "floor": floor}})),
        _ => unmapped("key:Replacement.LifeReduced.shape"),
    }
}
