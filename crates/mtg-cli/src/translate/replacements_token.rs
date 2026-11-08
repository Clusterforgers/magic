//! Token replacements: "if an effect would create one or more tokens under your control, it creates twice that many".

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// A `CreateToken` replacement that multiplies the tokens one effect makes for a player.
pub fn create_token(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let read = [
        "Event",
        "ActiveZones",
        "ValidToken",
        "ReplaceWith",
        "EffectOnly",
    ];
    check_keys(p, "Replacement.CreateToken", &read, |_, _| false)?;
    // Every token is made by an effect, so `EffectOnly$` changes nothing.
    let _ = p.get("EffectOnly");
    if p.get("ActiveZones") != Some("Battlefield") {
        return unmapped("replacement:CreateToken-zone");
    }
    let (controller, what) = whose_tokens(ctx, p.get("ValidToken").unwrap_or_default())?;
    let part = ctx.sub(p.get("ReplaceWith").unwrap_or_default())?;
    check_keys(
        &part,
        "Replacement.ReplaceToken",
        &["DB", "Type", "Amount"],
        |_, _| false,
    )?;
    let by = match (part.get("DB"), part.get("Type"), part.get("Amount")) {
        (Some("ReplaceToken"), Some("Amount"), None) => 2,
        (Some("ReplaceToken"), Some("Amount"), Some("Thrice")) => 3,
        _ => return unmapped("replacement:CreateToken-with"),
    };
    let mut when = json!({ "controller": controller });
    if !what.is_empty() {
        when["what"] = json!(what);
    }
    Ok(json!({"Replacement": {"when": {"TokenCreation": when}, "by": {"Multiply": by}}}))
}

/// The player a token is created for and the kind judged, from a word like `Creature.YouCtrl`.
fn whose_tokens(ctx: &Ctx, valid: &str) -> Tr<(&'static str, Vec<Value>)> {
    let Some((kind, owner)) = valid.split_once('.') else {
        return unmapped("replacement:CreateToken-ValidToken");
    };
    let controller = match owner {
        "YouCtrl" => "You",
        "OppCtrl" => "Opponents",
        _ => return unmapped("replacement:CreateToken-ValidToken"),
    };
    let what = if kind == "Card" {
        Vec::new()
    } else {
        selector_any(ctx, kind)?
    };
    Ok((controller, what))
}
