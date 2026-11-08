//! Token replacements: "if an effect would create one or more tokens under your control, it creates twice that many",
//! "those tokens plus an additional Food token are created instead" and "instead create one of each".

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// A `CreateToken` replacement: the tokens one effect makes for a player multiplied, joined by more tokens, or replaced.
pub fn create_token(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let read = [
        "Event",
        "ActiveZones",
        "ValidToken",
        "ValidPlayer",
        "ReplaceWith",
        "EffectOnly",
    ];
    check_keys(p, "Replacement.CreateToken", &read, |_, _| false)?;
    // Every token is made by an effect, so `EffectOnly$` changes nothing.
    let _ = p.get("EffectOnly");
    if p.get("ActiveZones") != Some("Battlefield") {
        return unmapped("replacement:CreateToken-zone");
    }
    let (controller, what) = whose_tokens(ctx, p.get("ValidPlayer"), p.get("ValidToken"))?;
    let part = ctx.sub(p.get("ReplaceWith").unwrap_or_default())?;
    check_keys(
        &part,
        "Replacement.ReplaceToken",
        &["DB", "Type", "Amount", "TokenScript"],
        |_, _| false,
    )?;
    let by = match (
        part.get("DB"),
        part.get("Type"),
        part.get("Amount"),
        part.get("TokenScript"),
    ) {
        (Some("ReplaceToken"), Some("Amount"), None, None) => json!({"Multiply": 2}),
        (Some("ReplaceToken"), Some("Amount"), Some("Thrice"), None) => json!({"Multiply": 3}),
        // "Those tokens plus an additional Food token": one more; with no amount, one more for each token ("that many").
        (Some("ReplaceToken"), Some("AddToken"), amount, Some(script)) => {
            let count = match amount {
                None => json!("Triggering"),
                Some("1") => json!(1),
                Some(_) => return unmapped("replacement:CreateToken-AddToken-amount"),
            };
            json!({ "Plus": made(ctx, script, &count)? })
        }
        // "Instead create one of each": each token of the kind becomes one of each listed.
        (Some("ReplaceToken"), Some("ReplaceToken"), None, Some(script)) if controller == "You" => {
            json!({ "Instead": made(ctx, script, &json!("Triggering"))? })
        }
        _ => return unmapped("replacement:CreateToken-with"),
    };
    let mut when = json!({ "controller": controller });
    if !what.is_empty() {
        when["what"] = json!(what);
    }
    Ok(json!({"Replacement": {"when": {"TokenCreation": when}, "by": by}}))
}

/// The tokens the scripts name, `count` of each, for the replacement's controller.
fn made(ctx: &mut Ctx, script: &str, count: &Value) -> Tr<Vec<Value>> {
    let line = Params::parse(&format!("DB$ Token | TokenScript$ {script}"));
    let mut effects = super::tokens::effects(ctx, &line, "Token")?;
    for effect in &mut effects {
        let Some(create) = effect.get_mut("CreateToken") else {
            return unmapped("replacement:CreateToken-made");
        };
        if count != &json!(1) {
            create["count"] = count.clone();
        }
    }
    Ok(effects)
}

/// The player a token is created for and the kind judged: `ValidToken$ Creature.YouCtrl`, or `ValidPlayer$ You` with a
/// kind such as `ValidToken$ Treasure`.
fn whose_tokens(
    ctx: &Ctx,
    player: Option<&str>,
    valid: Option<&str>,
) -> Tr<(&'static str, Vec<Value>)> {
    let (controller, kind) = match (player, valid) {
        (Some("You"), None) => ("You", "Card"),
        (Some("You"), Some(kind)) if !kind.contains('.') => ("You", kind),
        (None, Some(valid)) => {
            let Some((kind, owner)) = valid.split_once('.') else {
                return unmapped("replacement:CreateToken-ValidToken");
            };
            let controller = match owner {
                "YouCtrl" => "You",
                "OppCtrl" => "Opponents",
                _ => return unmapped("replacement:CreateToken-ValidToken"),
            };
            (controller, kind)
        }
        _ => return unmapped("replacement:CreateToken-ValidToken"),
    };
    let what = if kind == "Card" {
        Vec::new()
    } else {
        selector_any(ctx, kind)?
    };
    Ok((controller, what))
}
