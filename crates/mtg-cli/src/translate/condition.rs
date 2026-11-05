//! Conditions: `IsPresent$` with its zone and comparison, and `CheckSVar$`, as the DSL's `Condition`.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, unmapped};
use super::valid::{cmp, defined, selector_any, selector_in};
use crate::forge::script::Params;

/// The keys this module reads.
pub const KEYS: [&str; 11] = [
    "IsPresent",
    "PresentDefined",
    "PresentZone",
    "PresentPlayer",
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
    if defined_word.is_some() && p.has("PresentPlayer") {
        return unmapped("key:Condition.PresentPlayerWithDefined");
    }
    if let Some(word) = defined_word {
        return of_defined(ctx, word, valid, compare, zone).map(Some);
    }
    // The spell itself, on the stack or resolving: one if its caster promised the gift, else none.
    if valid == "Card.Self+PromisedGift" && matches!(zone, None | Some("Stack")) {
        let (op, right) = comparison(ctx, compare.unwrap_or("GE1"))?;
        return Ok(Some(
            json!({"Compare": {"left": "Promised", "op": op, "right": right}}),
        ));
    }
    let player = match p.get("PresentPlayer") {
        None => false,
        Some("You") => true,
        Some(other) => return unmapped(format!("key:Condition.PresentPlayer={other}")),
    };
    // "Battlefield,Graveyard" with the default count: it is there in any of the zones.
    if let Some(list) = zone.filter(|list| list.contains(',')) {
        let (op, right) = comparison(ctx, compare.unwrap_or("GE1"))?;
        if (op, &right) != ("Ge", &json!(1)) {
            return unmapped(format!("zone:{list}"));
        }
        let mut any = Vec::new();
        for each in list.split(',') {
            any.push(counted(ctx, valid, Some(each), player, "Ge", &right)?);
        }
        return Ok(Some(json!({ "Any": any })));
    }
    let (op, right) = comparison(ctx, compare.unwrap_or("GE1"))?;
    Ok(Some(counted(ctx, valid, zone, player, op, &right)?))
}

/// How many objects `valid` names in the zone compare this way; `player` limits them to those of "you".
fn counted(
    ctx: &Ctx,
    valid: &str,
    zone: Option<&str>,
    player: bool,
    op: &str,
    right: &Value,
) -> Tr<Value> {
    let mut selector = selector_in(ctx, valid, zone)?;
    let owner = |pred: &Value| pred.get("ControlledBy").or(pred.get("OwnedBy")).is_some();
    if player && !selector.iter().any(owner) {
        let outside = !matches!(zone, None | Some("Battlefield"));
        selector.push(if outside {
            json!({"OwnedBy": "You"})
        } else {
            json!({"ControlledBy": "You"})
        });
    }
    Ok(json!({"Compare": {"left": {"Count": selector}, "op": op, "right": right}}))
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
    let subject = match word {
        "Remembered" | "RememberedLKI" => {
            super::memory::slot_ref_loose(ctx, super::memory::Slot::Remembered)
                .map_or_else(|| unmapped("defined:Remembered-without-a-change"), Ok)?
        }
        _ => defined(ctx, word)?,
    };
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
    let each = super::header::per_player_targets(p)?;
    let p = each.as_ref().unwrap_or(p);
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

/// Revolt: a permanent left the battlefield under your control this turn.
pub fn revolt() -> Value {
    json!({"Compare": {"left": {"Tallied": {"who": "You", "what": "PermanentsLeftUnder"}}, "op": "Ge", "right": 1}})
}

/// A werewolf's upkeep condition: "no spells were cast last turn", or "a player cast two or more spells last turn".
pub fn spells_last_turn(two_by_one: bool) -> Value {
    let (op, right) = if two_by_one { ("Ge", 2) } else { ("Eq", 0) };
    json!({"Compare": {"left": {"CastLastTurn": "EachPlayer"}, "op": op, "right": right}})
}

/// Adamant: at least three mana of the color (`Any`: of one color, whichever) was spent to cast the spell.
pub fn adamant(color: &str) -> Tr<Value> {
    const COLORS: [&str; 5] = ["White", "Blue", "Black", "Red", "Green"];
    let three = |color: &str| json!({"Compare": {"left": {"ManaSpent": {"of": color}}, "op": "Ge", "right": 3}});
    match color {
        "Any" => Ok(json!({"Any": COLORS.map(three)})),
        known if COLORS.contains(&known) => Ok(three(known)),
        _ => unmapped("keyword:etbCounter-adamant-color"),
    }
}

/// The permanent itself has the way it was cast (`Card.Self+wasCastFromYourHandByYou`): a `Matches` on `This`.
pub fn self_was_cast(ctx: &Ctx, spec: &str) -> Tr<Value> {
    let selector = selector_any(ctx, spec)?;
    Ok(json!({"Matches": {"subject": "This", "selector": selector}}))
}

/// The line without the keys of its condition.
pub fn strip(p: &Params) -> Params {
    let mut keys = KEYS.to_vec();
    keys.extend(super::header::gate_words(p));
    p.without(&keys)
}
