//! `Mode$ AlternativeCost` on the spell itself: "you may pay X rather than pay this spell's mana cost" as a `FreeCast` with
//! a cost, or an `AltCast` when the card is cast from its graveyard.

use serde_json::{Value, json};

use super::cost::cost;
use super::ctx::{Ctx, Tr, is_hint, unmapped};
use crate::forge::script::Params;

/// The ability of an `AlternativeCost` line; the condition keys are already stripped from `p`.
pub fn alternative_cost(ctx: &Ctx, p: &Params) -> Tr<Value> {
    for (key, _) in p.iter() {
        let read = [
            "Mode",
            "ValidSA",
            "ValidCard",
            "ValidPlayer",
            "Cost",
            "EffectZone",
        ];
        if !read.contains(&key) && !is_hint(key) {
            return unmapped(format!("key:Static.AlternativeCost.{key}"));
        }
    }
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
