//! Effects that repeat for each player (`RepeatEach`) and that ask a player for a value (`ChooseColor`, `ChooseType`).

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, only, unmapped};
use super::effects::chain_at;
use super::valid::defined;
use crate::forge::script::Params;

/// `RepeatEach` over players: the sub-ability once for each, with the player bound as `Remembered`.
pub fn repeat_each(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "RepeatEach",
        &[
            "RepeatPlayers",
            "RepeatSubAbility",
            "DamageMap",
            "ChangeZoneTable",
        ],
    )?;
    let over = match p.get("RepeatPlayers") {
        Some("Player") => json!("EachPlayer"),
        Some("Player.Opponent" | "Opponent") => json!("Opponents"),
        Some("Targeted") => ctx.last_target()?,
        Some(other) => return unmapped(format!("key:RepeatEach.RepeatPlayers={other}")),
        None => return unmapped("key:RepeatEach.no-RepeatPlayers"),
    };
    let Some(sub) = p.get("RepeatSubAbility") else {
        return unmapped("key:RepeatEach.no-RepeatSubAbility");
    };
    let bind = ctx.fresh("each");
    let outer = ctx.remembered.replace(json!({ "Bound": bind }));
    let part = ctx.sub(sub)?;
    let effects = chain_at(ctx, &part, false);
    ctx.remembered = outer;
    Ok(vec![
        json!({"RepeatEach": {"over": over, "as": bind, "effects": effects?}}),
    ])
}

/// `ChooseColor` and `ChooseType`: the chooser picks a value that the later lines of the ability read as the chosen one.
pub fn choose_value(ctx: &mut Ctx, p: &Params, api: &str) -> Tr<Vec<Value>> {
    only(p, api, &["Defined", "Type"])?;
    let kind = match (api, p.get("Type")) {
        ("ChooseColor", None) => json!("Color"),
        ("ChooseType", Some("Creature")) => json!("CreatureType"),
        ("ChooseType", Some("Card")) => json!("CardType"),
        ("ChooseType", Some("Basic Land" | "Land")) => json!("LandType"),
        (_, other) => return unmapped(format!("key:{api}.Type={}", other.unwrap_or(""))),
    };
    let who = defined(ctx, p.get("Defined").unwrap_or("You"))?;
    let bind = ctx.fresh("chosen");
    ctx.chosen = Some(bind.clone());
    Ok(vec![
        json!({"ChooseValue": {"who": who, "kind": kind, "as": bind}}),
    ])
}
