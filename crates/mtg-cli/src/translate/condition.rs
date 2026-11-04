//! Conditions: `IsPresent$` with its zone and comparison, and `CheckSVar$`, as the DSL's `Condition`.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, unmapped};
use super::valid::{cmp, defined, selector_any, selector_in};
use crate::forge::script::Params;

/// The keys this module reads.
pub const KEYS: [&str; 7] = [
    "IsPresent",
    "PresentDefined",
    "PresentZone",
    "PresentCompare",
    "CheckSVar",
    "SVarCompare",
    "Condition",
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
        if let Some(word) = p.get("PresentDefined") {
            all.push(of_defined(ctx, word, valid, p.get("PresentCompare"), zone)?);
            return Ok(single(all));
        }
        let selector = selector_in(ctx, valid, zone)?;
        let (op, right) = comparison(ctx, p.get("PresentCompare").unwrap_or("GE1"))?;
        all.push(json!({"Compare": {"left": {"Count": selector}, "op": op, "right": right}}));
    } else if p.has("PresentZone") || p.has("PresentCompare") || p.has("PresentDefined") {
        return unmapped("key:Condition.PresentWithoutIsPresent");
    }
    if let Some(name) = p.get("CheckSVar") {
        let (op, right) = comparison(ctx, p.get("SVarCompare").unwrap_or("GE1"))?;
        all.push(json!({"Compare": {"left": amount(ctx, name)?, "op": op, "right": right}}));
    } else if p.has("SVarCompare") {
        return unmapped("key:Condition.SVarCompareWithoutCheckSVar");
    }
    if let Some(name) = p.get("Condition") {
        all.push(named(name)?);
    }
    Ok(single(all))
}

/// The conditions together: none, the one, or all of them.
fn single(mut all: Vec<Value>) -> Option<Value> {
    match all.len() {
        0 => None,
        1 => all.pop(),
        _ => Some(json!({ "All": all })),
    }
}

/// "How many of the objects `word` names are `valid`", compared (`ConditionDefined$ Remembered`): their zone is ignored.
fn of_defined(
    ctx: &Ctx,
    word: &str,
    valid: &str,
    compare: Option<&str>,
    zone: Option<&str>,
) -> Tr<Value> {
    if zone.is_some() {
        return unmapped("key:Condition.PresentZoneWithDefined");
    }
    let (op, right) = comparison(ctx, compare.unwrap_or("GE1"))?;
    let selector = selector_in(ctx, valid, None)?;
    let subject = defined(ctx, word)?;
    Ok(json!({"CountOf": {"subject": subject, "selector": selector, "op": op, "right": right}}))
}

/// The keys of a condition on one line of an ability chain and the keys of [`condition`] they stand for.
const PREFIXED: [(&str, &str); 6] = [
    ("ConditionDefined", "PresentDefined"),
    ("ConditionPresent", "IsPresent"),
    ("ConditionZone", "PresentZone"),
    ("ConditionCompare", "PresentCompare"),
    ("ConditionCheckSVar", "CheckSVar"),
    ("ConditionSVarCompare", "SVarCompare"),
];

/// The condition a line of a chain states with `ConditionPresent$` and the like, and the line without those keys.
pub fn prefixed(ctx: &Ctx, p: &Params) -> Tr<(Option<Value>, Params)> {
    let renamed: Vec<String> = PREFIXED
        .iter()
        .filter_map(|(from, to)| p.get(from).map(|value| format!("{to}$ {value}")))
        .collect();
    if renamed.is_empty() {
        return Ok((None, p.clone()));
    }
    let condition = condition(ctx, &Params::parse(&renamed.join(" | ")))?;
    let keys: Vec<&str> = PREFIXED.iter().map(|(from, _)| *from).collect();
    Ok((condition, p.without(&keys)))
}

/// A condition under which a permanent enters tapped, as the DSL states it: "unless it holds" (`Not(Compare Ge n)`) for
/// the comparisons that mean "fewer than".
pub fn tapped_when(condition: Value) -> Value {
    let Some(compare) = condition.get("Compare") else {
        return condition;
    };
    let (Some(op), Some(right)) = (
        compare.get("op").and_then(Value::as_str),
        compare.get("right").and_then(Value::as_i64),
    ) else {
        return condition;
    };
    let below = match (op, right) {
        ("Lt", n) => n,
        ("Le", n) => n + 1,
        ("Eq", 0) => 1,
        _ => return condition,
    };
    let mut atleast = condition.clone();
    atleast["Compare"]["op"] = json!("Ge");
    atleast["Compare"]["right"] = json!(below);
    json!({ "Not": atleast })
}

/// The condition a named Forge condition stands for (`Condition$ Threshold`).
pub fn named(name: &str) -> Tr<Value> {
    let yours = |extra: Value| vec![extra, json!({"OwnedBy": "You"})];
    Ok(match name {
        "PlayerTurn" => json!("YourTurn"),
        "NotPlayerTurn" => json!({"Not": "YourTurn"}),
        "Threshold" => {
            let cards = yours(json!({"In": "Graveyard"}));
            json!({"Compare": {"left": {"Count": cards}, "op": "Ge", "right": 7}})
        }
        "Delirium" => {
            let cards = yours(json!({"In": "Graveyard"}));
            json!({"Compare": {"left": {"CardTypes": cards}, "op": "Ge", "right": 4}})
        }
        "Hellbent" => {
            let cards = yours(json!({"In": "Hand"}));
            json!({"Compare": {"left": {"Count": cards}, "op": "Eq", "right": 0}})
        }
        "Metalcraft" => {
            let artifacts = vec![json!({"Type": "Artifact"}), json!({"ControlledBy": "You"})];
            json!({"Compare": {"left": {"Count": artifacts}, "op": "Ge", "right": 3}})
        }
        other => return unmapped(format!("condition:{other}")),
    })
}

/// Revolt: a permanent left the battlefield under your control this turn.
pub fn revolt() -> Value {
    json!({"Compare": {"left": {"Tallied": {"who": "You", "what": "PermanentsLeftUnder"}}, "op": "Ge", "right": 1}})
}

/// The permanent itself has the way it was cast (`Card.Self+wasCastFromYourHandByYou`): a `Matches` on `This`.
pub fn self_was_cast(ctx: &Ctx, spec: &str) -> Tr<Value> {
    let selector = selector_any(ctx, spec)?;
    Ok(json!({"Matches": {"subject": "This", "selector": selector}}))
}

/// The line without the keys of its condition.
pub fn strip(p: &Params) -> Params {
    p.without(&KEYS)
}
