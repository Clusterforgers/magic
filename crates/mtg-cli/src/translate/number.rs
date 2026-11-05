//! Numbers a step remembers: counters removed (`RememberRemoved$`, `RememberAmount$`) and the mana value of a countered
//! spell (`RememberCounteredCMC$`). The source remembers the number (`Effect::RememberValue`) and `Count$RememberedNumber`
//! and `Count$RememberedSize` read it back (`Amount::Remembered`).

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use crate::forge::script::Params;

/// The slot a remembered number lives in.
const SLOT: &str = "remembered_number";

/// How a line remembers a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The counters a `RemoveCounter` step took.
    Removed,
    /// The mana value of the spell a `Counter` step countered.
    CounteredValue,
}

/// The line without its number key, and the kind of number it remembers.
pub fn split(p: &Params, api: &str) -> Option<(Params, Kind)> {
    let (key, kind) = match api {
        "RemoveCounter" => (
            ["RememberRemoved", "RememberAmount"]
                .into_iter()
                .find(|key| p.get(key) == Some("True"))?,
            Kind::Removed,
        ),
        "Counter" => ("RememberCounteredCMC", Kind::CounteredValue),
        _ => return None,
    };
    (p.get(key) == Some("True")).then(|| (p.without(&[key]), kind))
}

/// The steps of a line with the number it remembers added, and the reading for later steps set on the context.
pub fn apply(ctx: &mut Ctx, kind: Kind, mut effects: Vec<Value>) -> Tr<Vec<Value>> {
    let at = match kind {
        Kind::Removed => effects.iter().position(|step| {
            step.get("RemoveCounters").is_some() || step.get("RemoveAllCounters").is_some()
        }),
        Kind::CounteredValue => effects
            .iter()
            .position(|step| step.get("Counter").is_some()),
    };
    let Some(at) = at else {
        return unmapped("key:RememberNumber-without-its-step");
    };
    let (value, at) = match kind {
        Kind::Removed => (removed(&effects[at])?, at),
        // Read once the spell is countered, as the spell it was.
        Kind::CounteredValue => {
            let Some(what) = effects[at].get("Counter").and_then(|c| c.get("what")) else {
                return unmapped("key:RememberCounteredCMC-without-a-spell");
            };
            (json!({"Stat": {"of": what, "stat": "ManaValue"}}), at + 1)
        }
    };
    effects.insert(at, json!({"RememberValue": {"value": value, "slot": SLOT}}));
    ctx.number = Some(json!({"Remembered": {"on": "This", "slot": SLOT}}));
    Ok(effects)
}

/// How many counters the removal takes, worked out before it runs: a count beyond what is there takes what is there.
fn removed(step: &Value) -> Tr<Value> {
    if let Some(all) = step.get("RemoveAllCounters") {
        return Ok(json!({"AnyCounters": all.get("from").cloned().unwrap_or(Value::Null)}));
    }
    let Some(body) = step.get("RemoveCounters") else {
        return unmapped("key:RememberRemoved-shape");
    };
    let from = body.get("from").cloned().unwrap_or(Value::Null);
    let held = match body.get("counter") {
        Some(kind) => json!({"Counters": {"on": from, "counter": kind}}),
        None => json!({"AnyCounters": from}),
    };
    let count = body.get("count").cloned().unwrap_or(json!(1));
    if count == held {
        return Ok(held);
    }
    let up_to = body.get("up_to").and_then(Value::as_bool).unwrap_or(false);
    match count.as_u64() {
        // The controller may take fewer than the count, which the engine decides; only a fixed take is known here.
        Some(n) if !up_to => Ok(json!({"When": {
            "condition": {"Compare": {"left": held, "op": "Ge", "right": n}},
            "then": n,
            "otherwise": held}})),
        _ => unmapped("key:RememberRemoved-amount"),
    }
}

/// `Count$RememberedNumber` and `Count$RememberedSize` after a step that remembered a number.
pub fn read(ctx: &Ctx) -> Option<Value> {
    ctx.number.clone()
}
