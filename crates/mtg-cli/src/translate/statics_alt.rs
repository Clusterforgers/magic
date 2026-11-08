//! `Mode$ AlternativeCost` on the spell itself: "you may pay X rather than pay this spell's mana cost" as a `FreeCast` with
//! a cost, or an `AltCast` when the card is cast from its graveyard; and `Mode$ OptionalCost`, the spell's own optional
//! additional cost.

use serde_json::{Value, json};

use super::cost::cost;
use super::ctx::{Ctx, Tr, check_keys, unmapped};
use crate::forge::script::Params;

/// The ability of an `AlternativeCost` line; the condition keys are already stripped from `p`.
pub fn alternative_cost(ctx: &Ctx, p: &Params) -> Tr<Value> {
    let read = [
        "Mode",
        "ValidSA",
        "ValidCard",
        "ValidPlayer",
        "Cost",
        "EffectZone",
    ];
    check_keys(p, "Static.AlternativeCost", &read, |key, _| key == "Mode")?;
    let on_self = matches!(
        (p.get("ValidSA"), p.get("ValidCard")),
        (Some("Spell.Self"), None) | (Some("Spell"), Some("Card.Self"))
    );
    if !on_self || !matches!(p.get("ValidPlayer"), None | Some("You")) {
        return unmapped("static:AlternativeCost-shape");
    }
    let paid = match p.get("Cost") {
        Some("0") => None,
        Some(text) => Some(cost(ctx, text)?),
        None => return unmapped("static:AlternativeCost-cost"),
    };
    match p.get("EffectZone") {
        None | Some("All" | "Hand") => {
            let mut ability = json!({});
            if let Some(paid) = paid {
                ability["cost"] = paid;
            }
            Ok(json!({ "FreeCast": ability }))
        }
        Some("Graveyard") => {
            let mut ability = json!({"way": "Alternative", "from": "Graveyard"});
            if let Some(paid) = paid {
                ability["cost"] = paid;
            }
            Ok(json!({ "AltCast": ability }))
        }
        Some(other) => unmapped(format!("key:Static.AlternativeCost.EffectZone={other}")),
    }
}

/// "As an additional cost to cast this spell, you may ...": the spell's own `OptionalCost` line as an `AdditionalCost` the
/// caster may skip. A line about other spells (the Defilers) is a gap.
pub fn optional_cost(ctx: &Ctx, p: &Params) -> Tr<Value> {
    let read = ["Mode", "ValidSA", "ValidCard", "Cost", "EffectZone"];
    check_keys(p, "Static.OptionalCost", &read, |key, _| key == "Mode")?;
    if p.get("ValidCard") != Some("Card.Self")
        || p.get("ValidSA") != Some("Spell")
        || !matches!(p.get("EffectZone"), None | Some("All"))
    {
        return unmapped("static:OptionalCost-of-other-spells");
    }
    match cost(ctx, p.get("Cost").unwrap_or_default())? {
        Value::Array(items) if !items.is_empty() => {
            Ok(json!({"AdditionalCost": {"cost": items, "required": false}}))
        }
        // Mana alone ("you may pay {2}{R}").
        Value::String(mana) if !mana.is_empty() => {
            Ok(json!({"AdditionalCost": {"cost": [{"Mana": mana}], "required": false}}))
        }
        _ => unmapped("static:OptionalCost-without-a-cost"),
    }
}
