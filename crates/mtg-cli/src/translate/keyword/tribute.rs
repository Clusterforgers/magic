//! Tribute N (CR 702.104): the opponent's choice as the creature enters and the trigger for when tribute wasn't paid.

use serde_json::{Value, json};

use crate::translate::ctx::{Ctx, Tr, unmapped};
use crate::translate::effects::chain_at;

/// `Tribute:N`: the replacement that asks an opponent, and "when this enters, if tribute wasn't paid" with the effects of
/// the card's `TrigNotTribute` part. A tribute without that part is the replacement alone.
pub fn tribute(ctx: &mut Ctx, count: &str) -> Vec<Tr<Value>> {
    let Ok(count) = count.parse::<u32>() else {
        return vec![unmapped("keyword:Tribute-shape")];
    };
    let mut found = vec![Ok(
        json!({"Keyword": {"Macro": {"name": "tribute", "amount": count}}}),
    )];
    if ctx.svar("TrigNotTribute").is_some() {
        found.push(crate::translate::card::audited_one(|| unpaid(ctx)));
    }
    found
}

fn unpaid(ctx: &mut Ctx) -> Tr<Value> {
    ctx.reset();
    let part = ctx.sub("TrigNotTribute")?;
    let effects = chain_at(ctx, &part, false)?;
    let mut ability = json!({
        "when": {"ZoneChange": {"what": ["This"], "to": "Battlefield"}},
        "if": {"Compare": {"left": "Chosen", "op": "Eq", "right": 0}},
        "effects": effects});
    if !ctx.targets.is_empty() {
        ability["targets"] = Value::Array(ctx.targets.clone());
    }
    Ok(json!({ "Triggered": ability }))
}
