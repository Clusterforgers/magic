//! `Mode$ OptionalCost`: "As an additional cost to cast this spell, you may ..." on the spell itself, as an
//! `AdditionalCost` the caster may skip (CR 601.2b), read back by `Condition::AdditionalCostPaid`.

use serde_json::{Value, json};

use super::cost::cost;
use super::ctx::{Ctx, Tr, check_keys, unmapped};
use crate::forge::script::Params;

/// The ability of an `OptionalCost` line of the spell itself.
pub fn optional_cost(ctx: &Ctx, p: &Params) -> Tr<Value> {
    let read = ["Mode", "EffectZone", "ValidCard", "ValidSA", "Cost"];
    check_keys(p, "Static.OptionalCost", &read, |key, _| key == "Mode")?;
    let own = p.get("ValidCard") == Some("Card.Self")
        && p.get("ValidSA") == Some("Spell")
        && matches!(p.get("EffectZone"), None | Some("All"));
    if !own {
        return unmapped("static:OptionalCost-of-other-spells");
    }
    let Some(text) = p.get("Cost") else {
        return unmapped("static:OptionalCost-without-a-cost");
    };
    Ok(json!({"AdditionalCost": {"cost": cost(ctx, text)?}}))
}
