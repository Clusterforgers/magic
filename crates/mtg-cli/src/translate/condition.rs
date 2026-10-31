//! Conditions: `IsPresent$` with its zone and comparison, and `CheckSVar$`, as the DSL's `Condition`.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, unmapped};
use super::valid::{cmp, selector_in};
use crate::forge::script::Params;

/// The keys this module reads.
pub const KEYS: [&str; 5] = [
    "IsPresent",
    "PresentZone",
    "PresentCompare",
    "CheckSVar",
    "SVarCompare",
];

/// A comparison word (`GE2`, `EQ0`, `LTX`) as the DSL's operator and amount.
fn comparison(ctx: &Ctx, word: &str) -> Tr<(&'static str, Value)> {
    let Some(op) = word.get(..2).and_then(cmp) else {
        return unmapped(format!("compare:{word}"));
    };
    Ok((op, amount(ctx, word.get(2..).unwrap_or_default())?))
}

/// The condition the keys of a line state, `None` when it has none; the line without them is [`strip`].
pub fn condition(ctx: &Ctx, p: &Params) -> Tr<Option<Value>> {
    let mut all = Vec::new();
    if let Some(valid) = p.get("IsPresent") {
        let zone = p.get("PresentZone");
        let selector = selector_in(ctx, valid, zone)?;
        let (op, right) = comparison(ctx, p.get("PresentCompare").unwrap_or("GE1"))?;
        all.push(json!({"Compare": {"left": {"Count": selector}, "op": op, "right": right}}));
    } else if p.has("PresentZone") || p.has("PresentCompare") {
        return unmapped("key:Condition.PresentWithoutIsPresent");
    }
    if let Some(name) = p.get("CheckSVar") {
        let (op, right) = comparison(ctx, p.get("SVarCompare").unwrap_or("GE1"))?;
        all.push(json!({"Compare": {"left": amount(ctx, name)?, "op": op, "right": right}}));
    } else if p.has("SVarCompare") {
        return unmapped("key:Condition.SVarCompareWithoutCheckSVar");
    }
    Ok(match all.len() {
        0 => None,
        1 => all.pop(),
        _ => Some(json!({ "All": all })),
    })
}

/// The line without the keys of its condition.
pub fn strip(p: &Params) -> Params {
    p.without(&KEYS)
}
