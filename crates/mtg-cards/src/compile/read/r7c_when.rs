//! A condition held as a number (r7c): "the number that is 1 when C holds, else 0" compared with 1 or 0 is C itself or
//! its negation. A rule no writer writes is written again with each such comparison replaced by the condition, so
//! "If you're the monarch, ... instead" reads back whichever way the rule holds the test. A count of what the step before
//! sacrificed or exiled, one or more, is "you do" in the same way.

use serde_json::{Value, json};

use crate::ability::Ability;

/// The condition a comparison of the number of a condition stands for: the condition when the number is 1, its
/// negation when it is 0. Any other comparison, or numbers other than 1 and 0, stays.
fn plain(value: &Value) -> Option<Value> {
    let compare = value.get("Compare")?;
    let when = compare.get("left")?.get("When")?;
    (when.get("then")? == &json!(1) && when.get("otherwise")? == &json!(0)).then_some(())?;
    let condition = when.get("condition")?.clone();
    let holds = match (
        compare.get("op")?.as_str()?,
        compare.get("right")?.as_i64()?,
    ) {
        ("Ge" | "Eq", 1) | ("Gt", 0) => true,
        ("Eq" | "Le", 0) | ("Lt", 1) => false,
        _ => return None,
    };
    Some(if holds {
        condition
    } else {
        json!({ "Not": condition })
    })
}

/// Whether the step sacrifices or exiles for the controller and binds what it moved under `bind`.
fn moves_as(step: &Value, bind: &Value) -> bool {
    ["Sacrifice", "Exile"].iter().any(|verb| {
        step.get(verb).is_some_and(|body| {
            body.get("as") == Some(bind)
                && body.get("who").is_none()
                && body.get("duration").is_none()
        })
    })
}

/// "If you do" after a sacrifice or an exile: the condition that one or more of the objects that step moved were moved
/// is the condition that the step was carried out (`Performed`), in the step right after it.
fn performed(steps: &mut [Value]) -> bool {
    let mut found = false;
    for at in 1..steps.len() {
        let (before, after) = steps.split_at_mut(at);
        let Some(condition) = after[0]
            .get_mut("If")
            .and_then(|body| body.get_mut("condition"))
        else {
            continue;
        };
        let Some(count) = condition.get("CountOf") else {
            continue;
        };
        let bind = count
            .get("subject")
            .and_then(|subject| subject.get("Bound"))
            .cloned();
        let one_or_more = count.get("op") == Some(&json!("Ge"))
            && count.get("right") == Some(&json!(1))
            && count.get("selector") == Some(&json!([]));
        if let Some(bind) = bind
            && one_or_more
            && moves_as(&before[at - 1], &bind)
        {
            *condition = json!("Performed");
            found = true;
            // "When you do": a reflexive trigger is put on the stack only when the step before was carried out already.
            let then = after[0]["If"].get("then").and_then(Value::as_array);
            if let Some([reflexive]) = then.map(Vec::as_slice)
                && reflexive.get("Reflexive").is_some()
                && after[0]["If"].get("otherwise").is_none()
            {
                after[0] = reflexive.clone();
            }
        }
    }
    found
}

/// The variant names of the effects of a list and of the steps inside them.
fn heads(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                out.push(key.clone());
                heads(inner, out);
            }
        }
        Value::Array(list) => list.iter().for_each(|inner| heads(inner, out)),
        _ => {}
    }
}

/// Whether `steps` leave the condition as it was: a fact of the cast, or a fact the steps have no part in.
fn stable(condition: &Value, steps: &Value) -> bool {
    let mut names = Vec::new();
    heads(steps, &mut names);
    let has = |name: &str| names.iter().any(|found| found == name);
    if ["AdditionalCostPaid", "Kicked"].contains(&condition.as_str().unwrap_or_default()) {
        return true;
    }
    if condition.get("CitysBlessing").is_some() {
        return !has("CreateToken") && !has("Battlefield");
    }
    if condition.get("Monarch").is_some() {
        return !has("BecomeMonarch");
    }
    let Some(left) = condition
        .get("Compare")
        .and_then(|compare| compare.get("left"))
    else {
        return false;
    };
    let cast = *left == json!("Promised")
        || left.get("ManaSpent").is_some()
        || left.get("Stat").and_then(|stat| stat.get("stat")) == Some(&json!("ManaSpent"));
    let lands =
        left.get("Tallied").and_then(|tally| tally.get("what")) == Some(&json!("LandsPlayed"));
    // The card types in a graveyard change only when a card goes to or leaves a graveyard.
    let types = left.get("CardTypes").is_some()
        && !names.iter().any(|name| {
            matches!(
                name.as_str(),
                "Graveyard"
                    | "Discard"
                    | "Mill"
                    | "Destroy"
                    | "Sacrifice"
                    | "Exile"
                    | "DealDamage"
                    | "Fight"
                    | "Continuous"
                    | "Counter"
                    | "Surveil"
            )
        });
    cast || lands || types
}

/// "S1. If C, instead S2.": a step done unless the condition holds and the step done if it does, the first leaving the
/// condition as it was, are one step with two branches.
fn merged(steps: &mut Vec<Value>) -> bool {
    let mut found = false;
    let mut at = 0;
    while at + 1 < steps.len() {
        let (first, second) = (&steps[at]["If"], &steps[at + 1]["If"]);
        let pair = first.is_object()
            && second.is_object()
            && first.get("otherwise").is_none()
            && second.get("otherwise").is_none()
            && second.get("condition").is_some_and(|condition| {
                first.get("condition") == Some(&json!({ "Not": condition }))
                    && first
                        .get("then")
                        .is_some_and(|then| stable(condition, then))
            });
        if pair {
            let otherwise = first["then"].clone();
            let mut joined = steps[at + 1].clone();
            joined["If"]["otherwise"] = otherwise;
            steps[at] = joined;
            steps.remove(at + 1);
            found = true;
        }
        at += 1;
    }
    found
}

/// Replaces each comparison of the number of a condition in the value, and each count of what the step before moved;
/// whether one was replaced.
fn replace(value: &mut Value) -> bool {
    if let Some(condition) = plain(value) {
        *value = condition;
        return true;
    }
    match value {
        Value::Object(map) => map.values_mut().fold(false, |found, v| replace(v) | found),
        Value::Array(list) => {
            let inner = list.iter_mut().fold(false, |found, v| replace(v) | found);
            performed(list) | inner
        }
        _ => false,
    }
}

/// Joins each step and its replacement in the value into one step with two branches; whether one was joined.
fn merge_all(value: &mut Value) -> bool {
    match value {
        Value::Object(map) => map
            .values_mut()
            .fold(false, |found, v| merge_all(v) | found),
        Value::Array(list) => {
            let inner = list.iter_mut().fold(false, |found, v| merge_all(v) | found);
            merged(list) | inner
        }
        _ => false,
    }
}

/// The ability with each step and its replacement joined ("S1. If C, instead S2."), its conditions held as numbers
/// replaced first; `None` when it has no such pair.
pub(super) fn r7c_instead(ability: &Ability) -> Option<Ability> {
    let mut value = serde_json::to_value(ability).ok()?;
    replace(&mut value);
    merge_all(&mut value).then_some(())?;
    serde_json::from_value(value).ok()
}

/// The ability with each comparison of the number of a condition replaced by the condition; `None` when it has none.
pub(super) fn r7c_unwrapped(ability: &Ability) -> Option<Ability> {
    let mut value = serde_json::to_value(ability).ok()?;
    replace(&mut value).then_some(())?;
    serde_json::from_value(value).ok()
}
