//! `OptionalAttackCost` with an exert cost: the creature may be exerted as it attacks, and the trigger it names fires on the exert.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use crate::forge::script::Params;

/// The abilities of an exert line (the keyword, then the trigger when the line names one); `None` for any other mode.
pub fn exert(ctx: &mut Ctx, p: &Params) -> Option<Tr<Value>> {
    if p.get("Mode") != Some("OptionalAttackCost") {
        return None;
    }
    Some(abilities(ctx, p))
}

fn abilities(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    check_keys(
        p,
        "Static.OptionalAttackCost",
        &["ValidCard", "Cost", "Trigger", "IsPresent"],
        |key, value| key == "Mode" || (key == "EffectZone" && value == "Battlefield"),
    )?;
    if p.get("ValidCard") != Some("Card.Self") {
        return unmapped("key:Static.OptionalAttackCost.ValidCard");
    }
    if p.get("Cost") != Some("Exert<1/CARDNAME>") {
        return unmapped("key:Static.OptionalAttackCost.Cost");
    }
    let once_per_turn = match p.get("IsPresent") {
        None => false,
        Some("Creature.Self+notExertedThisTurn") => true,
        Some(_) => return unmapped("key:Static.OptionalAttackCost.IsPresent"),
    };
    let keyword = if once_per_turn {
        json!({"Keyword": {"Exert": {"once_per_turn": true}}})
    } else {
        json!({"Keyword": {"Exert": {}}})
    };
    let Some(execute) = p.get("Trigger") else {
        return Ok(keyword);
    };
    let line = format!("Mode$ Exerted | ValidCard$ Card.Self | Execute$ {execute}");
    let trigger = super::triggers::triggered(ctx, &line)?;
    Ok(json!([keyword, trigger]))
}
