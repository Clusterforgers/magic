//! Conditions: `IsPresent$` with its zone and comparison, and `CheckSVar$`, as the DSL's `Condition`.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, unmapped};
use super::valid::{cmp, defined, selector_in};
use crate::forge::script::Params;

/// The keys this module reads.
pub const KEYS: [&str; 10] = [
    "IsPresent",
    "PresentDefined",
    "PresentZone",
    "PresentCompare",
    "IsPresent2",
    "PresentZone2",
    "PresentCompare2",
    "CheckSVar",
    "SVarCompare",
    "Condition",
];

/// A comparison word (`GE2`, `EQ0`, `LTX`) as the DSL's operator and amount.
pub(super) fn comparison(ctx: &Ctx, word: &str) -> Tr<(&'static str, Value)> {
    let Some(op) = word.get(..2).and_then(cmp) else {
        return unmapped(format!("compare:{word}"));
    };
    Ok((op, amount(ctx, word.get(2..).unwrap_or_default())?))
}

/// The condition the keys of a line state, `None` when it has none; the line without them is [`strip`].
pub fn condition(ctx: &Ctx, p: &Params) -> Tr<Option<Value>> {
    let mut all = Vec::new();
    for suffix in ["", "2"] {
        all.extend(present(ctx, p, suffix)?);
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
    all.extend(super::header::trigger_gates(p)?);
    Ok(single(all))
}

/// The condition of one `IsPresent` clause (`suffix` is empty for the first and `2` for the second), `None` without one.
fn present(ctx: &Ctx, p: &Params, suffix: &str) -> Tr<Option<Value>> {
    let key = |name: &str| format!("{name}{suffix}");
    let zone = p.get(&key("PresentZone"));
    let compare = p.get(&key("PresentCompare"));
    let defined_word = if suffix.is_empty() {
        p.get("PresentDefined")
    } else {
        None
    };
    let Some(valid) = p.get(&key("IsPresent")) else {
        if zone.is_some() || compare.is_some() || defined_word.is_some() {
            return unmapped("key:Condition.PresentWithoutIsPresent");
        }
        return Ok(None);
    };
    if let Some(word) = defined_word {
        return of_defined(ctx, word, valid, compare, zone).map(Some);
    }
    let selector = selector_in(ctx, valid, zone)?;
    let (op, right) = comparison(ctx, compare.unwrap_or("GE1"))?;
    Ok(Some(
        json!({"Compare": {"left": {"Count": selector}, "op": op, "right": right}}),
    ))
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
const PREFIXED: [(&str, &str); 10] = [
    ("Condition", "Condition"),
    ("ConditionDefined", "PresentDefined"),
    ("ConditionPresent", "IsPresent"),
    ("ConditionZone", "PresentZone"),
    ("ConditionCompare", "PresentCompare"),
    ("ConditionPresent2", "IsPresent2"),
    ("ConditionZone2", "PresentZone2"),
    ("ConditionCompare2", "PresentCompare2"),
    ("ConditionCheckSVar", "CheckSVar"),
    ("ConditionSVarCompare", "SVarCompare"),
];

/// Whether the line is the `Effect` that holds a shield for the source a `ChooseSource` picked and states "if a source was
/// chosen": a shield that names no object does nothing, so the condition adds nothing.
fn holds_a_shield_for_the_choice(p: &Params) -> bool {
    p.get("DB") == Some("Effect")
        && p.has("ReplacementEffects")
        && p.get("ConditionDefined") == Some("ChosenCard")
        && matches!(p.get("ConditionPresent"), Some("Card" | "Card,Emblem"))
        && matches!(p.get("ConditionCompare"), None | Some("GE1"))
}

/// The condition a line of a chain states with `ConditionPresent$` and the like, and the line without those keys.
pub fn prefixed(ctx: &Ctx, p: &Params) -> Tr<(Option<Value>, Params)> {
    let zoned = super::header::zone_targets(p);
    let p = zoned.as_ref().unwrap_or(p);
    let uniqued = super::header::unique_targets(ctx, p)?;
    let p = uniqued.as_ref().unwrap_or(p);
    if holds_a_shield_for_the_choice(p) {
        let keys: Vec<&str> = PREFIXED.iter().map(|(from, _)| *from).collect();
        return Ok((None, p.without(&keys)));
    }
    let mut renamed: Vec<String> = PREFIXED
        .iter()
        .filter_map(|(from, to)| p.get(from).map(|value| format!("{to}$ {value}")))
        .collect();
    // "If none is present": the same clause compared with zero.
    if let Some(valid) = p.get("ConditionNotPresent") {
        if p.has("ConditionPresent") || p.has("ConditionCompare") {
            return unmapped("key:Condition.NotPresentBeside");
        }
        renamed.push(format!("IsPresent$ {valid} | PresentCompare$ EQ0"));
    }
    let mut keys: Vec<&str> = PREFIXED.iter().map(|(from, _)| *from).collect();
    keys.extend(super::header::GATE_KEYS);
    let mut all: Vec<Value> = condition(ctx, &Params::parse(&renamed.join(" | ")))?
        .into_iter()
        .collect();
    all.extend(super::header::gate_conditions(ctx, p)?);
    Ok((single(all), p.without(&keys)))
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
        "Kicked" => json!("Kicked"),
        "Blessing" => json!({"CitysBlessing": "You"}),
        "Monarch" => json!({"Monarch": "You"}),
        "Night" => json!("IsNight"),
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

/// The line without the keys of its condition.
pub fn strip(p: &Params) -> Params {
    let mut keys = KEYS.to_vec();
    keys.extend(super::header::gate_words(p));
    p.without(&keys)
}
