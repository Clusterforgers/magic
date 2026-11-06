//! Replacements of the mana a permanent adds when it is tapped for mana (`Event$ ProduceMana`): the type, the amount or a
//! color changes, as a rule of the permanent or of an effect.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The keys of the replacement line a rule reads.
const READ: [&str; 6] = [
    "Event",
    "ValidCard",
    "ValidActivator",
    "ReplaceWith",
    "ActiveZones",
    "Secondary",
];

/// `Event$ ProduceMana` on a permanent: the rule `ManaProduced`, in force while the permanent is in play.
pub fn produce_mana(ctx: &mut Ctx, p: &Params, line_condition: Option<&Value>) -> Tr<Value> {
    if line_condition.is_some() {
        return unmapped("key:Replacement.ProduceMana.condition");
    }
    if p.get("ActiveZones") != Some("Battlefield") {
        return unmapped("key:Replacement.ProduceMana.ActiveZones");
    }
    Ok(json!({"Rule": {"rule": rule(ctx, p)?}}))
}

/// The `GameRule::ManaProduced` a `ProduceMana` replacement line stands for.
pub fn rule(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    check_keys(p, "Replacement.ProduceMana", &READ, |_, _| false)?;
    if !matches!(p.get("ActiveZones"), Some("Battlefield" | "Command")) {
        return unmapped("key:Replacement.ProduceMana.ActiveZones");
    }
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Permanent"))?;
    let who = match p.get("ValidActivator") {
        None | Some("Player") => "EachPlayer",
        Some("You") => "You",
        Some(_) => return unmapped("key:Replacement.ProduceMana.ValidActivator"),
    };
    let Some(name) = p.get("ReplaceWith") else {
        return unmapped("replacement:ProduceMana-without-ReplaceWith");
    };
    let part = ctx.sub(name)?;
    let change = change(&part)?;
    Ok(json!({"ManaProduced": {"what": what, "who": who, "change": change}}))
}

/// The change a `DB$ ReplaceMana` part makes.
fn change(part: &Params) -> Tr<Value> {
    check_keys(
        part,
        "Replacement.ProduceMana.Replace",
        &[
            "DB",
            "ReplaceMana",
            "ReplaceType",
            "ReplaceAmount",
            "ReplaceColor",
            "ReplaceOnly",
        ],
        |_, _| false,
    )?;
    if part.get("DB") != Some("ReplaceMana") {
        return unmapped("replacement:ProduceMana-instead");
    }
    match (
        part.get("ReplaceMana"),
        part.get("ReplaceType"),
        part.get("ReplaceAmount"),
        part.get("ReplaceColor"),
        part.get("ReplaceOnly"),
    ) {
        (Some(kind), None, None, None, None) => Ok(json!({"One": kind_of(kind)?})),
        (None, Some(kind), None, None, None) => Ok(json!({"OnlyType": kind_of(kind)?})),
        (None, None, Some(times), None, None) => match times.parse::<u32>() {
            Ok(times) if times >= 2 => Ok(json!({"Times": times})),
            _ => unmapped("key:Replacement.ProduceMana.ReplaceAmount"),
        },
        (None, None, None, Some(to), Some(from)) => {
            Ok(json!({"Convert": {"from": kind_of(from)?, "to": kind_of(to)?}}))
        }
        _ => unmapped("replacement:ProduceMana-choice"),
    }
}

/// The mana type a Forge letter or color word names; the player's choice (`Any`) and the chosen color are gaps.
fn kind_of(word: &str) -> Tr<&'static str> {
    Ok(match word {
        "W" | "White" => "White",
        "U" | "Blue" => "Blue",
        "B" | "Black" => "Black",
        "R" | "Red" => "Red",
        "G" | "Green" => "Green",
        "C" | "Colorless" => "Colorless",
        _ => return unmapped("replacement:ProduceMana-type"),
    })
}

/// An `Effect` line whose only content is a `ProduceMana` replacement ("Until end of turn, if you tap a land for mana, it
/// produces {U} instead of any other type"): the rule for the length of its duration.
pub fn effect(ctx: &mut Ctx, p: &Params, name: &str) -> Tr<Vec<Value>> {
    let sub = ctx.sub(name)?;
    let rule = rule(ctx, &sub)?;
    let duration = super::effects_obj::duration(p.get("Duration"))?;
    Ok(vec![json!({"Rule": {"rule": rule, "duration": duration}})])
}
