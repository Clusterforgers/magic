//! `GameLoss` replacements that forbid the loss: "you can't lose the game".

use serde_json::{Value, json};

use super::condition::{KEYS, condition};
use super::ctx::{Ctx, Tr, check_keys, unmapped};
use crate::forge::script::Params;

/// A `GameLoss` line with `Layer$ CantHappen` for the controller, under the line's own condition.
pub fn game_loss(ctx: &Ctx, p: &Params, prefixed: Option<&Value>) -> Tr<Value> {
    let mut read = vec!["Event", "ActiveZones", "ValidPlayer", "Layer"];
    read.extend(KEYS);
    check_keys(p, "Replacement.GameLoss", &read, |_, _| false)?;
    if prefixed.is_some() {
        return unmapped("key:Replacement.GameLoss.condition");
    }
    let condition = condition(ctx, p)?;
    if p.get("Layer") != Some("CantHappen") {
        return unmapped("replacement:GameLoss-instead");
    }
    if p.get("ValidPlayer") != Some("You") {
        return unmapped("replacement:GameLoss-ValidPlayer");
    }
    if !matches!(p.get("ActiveZones"), None | Some("Battlefield")) {
        return unmapped("replacement:GameLoss-zone");
    }
    // The line's text says whether the opponents also can't win; a different text is a different rule.
    let opponents_cant_win = match p.get("Description").map(str::trim) {
        Some("You can't lose the game and your opponents can't win the game.") => true,
        Some("You can't lose the game.") => false,
        _ => return unmapped("replacement:GameLoss-text"),
    };
    let mut rule = json!({"CantLoseGame": {"who": "You"}});
    if opponents_cant_win {
        rule["CantLoseGame"]["opponents_cant_win"] = json!(true);
    }
    let mut out = json!({"Rule": {"rule": rule}});
    if let Some(condition) = condition {
        out["Rule"]["condition"] = condition;
    }
    Ok(out)
}
