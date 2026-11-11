//! "As an additional cost to cast this spell, you may exile any number of white cards from your hand. This spell costs
//! {2} less to cast for each card exiled this way." and "you may pay {1}{G} any number of times": a `RaiseCost` of the
//! spell itself whose count is a variable the spell announces (`Announce$`), read as a repeatable additional cost
//! (`AdditionalCost.repeatable`) whose payments `AdditionalPaid` counts.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use crate::forge::script::Params;

/// The variable a "you may ... any number of" cost line counts, when the face has exactly one such line and one ability
/// announces the variable; it holds `Number$0` until the payments set it.
pub fn ep30_scan(ctx: &Ctx, statics: &[String], abilities: &[String]) -> Option<String> {
    let mut found = None;
    for body in statics {
        let p = Params::parse(body);
        if !is_any_number_line(&p) {
            continue;
        }
        let var = counted_var(ctx, &p)?;
        if found.replace(var).is_some() {
            return None;
        }
    }
    let var = found?;
    let announced = abilities
        .iter()
        .filter(|body| Params::parse(body).get("Announce") == Some(var.as_str()))
        .count();
    (announced == 1 && ctx.svar(&var).map(str::trim) == Some("Number$0")).then_some(var)
}

/// The ability line without the announcement of `var` and its prompt, which the repeatable cost asks instead.
pub fn ep30_unannounced(body: &str, var: &str) -> String {
    body.split(" | ")
        .filter(|part| {
            let part = part.trim();
            part != format!("Announce$ {var}") && !part.starts_with("AnnounceTitle$")
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// A `RaiseCost` of the spell itself that the caster may pay any number of times.
fn is_any_number_line(p: &Params) -> bool {
    p.get("Mode") == Some("RaiseCost")
        && p.get("ValidCard") == Some("Card.Self")
        && p.get("Type") == Some("Spell")
        && p.get("Description")
            .is_some_and(|text| text.contains("you may") && text.contains("any number of"))
}

/// The variable a cost line counts: `Amount$` of a mana cost, or the count of its one part (`ExileFromHand<Y/...>`),
/// through one `SVar$` alias.
fn counted_var(ctx: &Ctx, p: &Params) -> Option<String> {
    let cost = p.get("Cost")?;
    let name = match p.get("Amount") {
        Some(name) => name.to_owned(),
        None => {
            let (_, rest) = cost.split_once('<')?;
            rest.split('/').next()?.to_owned()
        }
    };
    if name.parse::<i64>().is_ok() {
        return None;
    }
    Some(
        match ctx
            .svar(&name)
            .map(str::trim)
            .and_then(|value| value.strip_prefix("SVar$"))
        {
            Some(alias) if !alias.contains('/') => alias.to_owned(),
            _ => name,
        },
    )
}

/// The repeatable additional cost of the spell, one unit paid each time: `Cost$` with its count set to one.
pub fn ep30_repeatable_cost(ctx: &Ctx, p: &Params) -> Tr<Value> {
    let Some(var) = ctx.ep30_paid.as_deref() else {
        return unmapped("key:Static.CostModifier.Cost");
    };
    // A spell's own cost applies in every zone it is cast from.
    if !is_any_number_line(p)
        || counted_var(ctx, p).as_deref() != Some(var)
        || !matches!(p.get("EffectZone"), None | Some("All"))
    {
        return unmapped("key:Static.CostModifier.Cost");
    }
    let cost = p.get("Cost").unwrap_or_default();
    let unit = match cost.split_once('<') {
        Some((head, rest)) if p.get("Amount").is_none() => {
            let (_, tail) = rest.split_once('/').unwrap_or((rest, ""));
            format!("{head}<1/{tail}")
        }
        Some(_) => return unmapped("key:Static.CostModifier.Cost-with-Amount"),
        None => cost.to_owned(),
    };
    let items = match super::cost::cost(ctx, &unit)? {
        Value::Array(items) => items,
        // Mana alone ("pay {1}{G} any number of times").
        Value::String(mana) if !mana.is_empty() => vec![json!({ "Mana": mana })],
        _ => return unmapped("key:Static.CostModifier.Cost"),
    };
    if items.len() != 1 {
        return unmapped("key:Static.CostModifier.Cost");
    }
    Ok(json!({"AdditionalCost": {"cost": items, "repeatable": true}}))
}
