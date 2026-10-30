//! Rules a spell sets beside a keyword grant (checker only): "Until end of turn, your life total can't change, and
//! permanents you control gain hexproof and indestructible." and "You and permanents you control gain hexproof until
//! end of turn. If the gift was promised, permanents you control also gain indestructible until end of turn."

use serde_json::{Value, json};

use super::w4d_parts::{parts, with_effects};
use crate::ability::Ability;

/// The rule of a spell that lasts until end of turn.
fn until_end_of_turn(rule: &Value) -> Value {
    json!({"Rule": {"duration": "UntilEndOfTurn", "rule": rule}})
}

/// The keywords a grant until end of turn gives, when the step is a `Continuous` of that duration.
fn grant(step: &Value) -> Option<&Value> {
    let inner = step.get("Continuous")?;
    (inner.get("duration") == Some(&json!("UntilEndOfTurn"))).then_some(inner)
}

/// Adds the wordings of a rule step and a keyword grant of a spell.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((value, effects)) = parts(ability) else {
        return;
    };
    if value.get("Spell").is_none() {
        return;
    }
    match effects.as_slice() {
        [rule, step] if *rule == until_end_of_turn(&json!({"LifeTotalCantChange": "You"})) => {
            texts.extend(life_total_and_grant(&value, step));
        }
        [rule, step] => texts.extend(you_and_group(&value, rule, step, None)),
        [rule, step, promised] => texts.extend(you_and_group(&value, rule, step, Some(promised))),
        _ => {}
    }
}

/// "Until end of turn, your life total can't change, and <the grant>."
fn life_total_and_grant(value: &Value, step: &Value) -> Vec<String> {
    if grant(step).is_none() {
        return Vec::new();
    }
    with_effects(value, std::slice::from_ref(step))
        .iter()
        .filter_map(|text| text.strip_prefix("Until end of turn, "))
        .map(|rest| format!("Until end of turn, your life total can't change, and {rest}"))
        .collect()
}

/// "You and permanents you control gain hexproof until end of turn." for a player who has the keyword the group gains,
/// and "If the gift was promised, <group> also gains Y." for a second grant under the condition.
fn you_and_group(
    value: &Value,
    rule: &Value,
    step: &Value,
    promised: Option<&Value>,
) -> Vec<String> {
    let Some(inner) = grant(step) else {
        return Vec::new();
    };
    let Some(keyword) = rule.pointer("/Rule/rule/PlayersHave/keyword") else {
        return Vec::new();
    };
    let same_player = rule.pointer("/Rule/rule/PlayersHave/who") == Some(&json!("You"));
    let same_duration = rule.pointer("/Rule/duration") == Some(&json!("UntilEndOfTurn"));
    let mods = inner.get("mods").and_then(Value::as_array);
    if !(same_player && same_duration && mods == Some(&vec![json!({"AddKeyword": keyword})])) {
        return Vec::new();
    }
    let first: Vec<String> = with_effects(value, std::slice::from_ref(step))
        .iter()
        .filter_map(|text| text.strip_prefix("Permanents "))
        .map(|rest| format!("You and permanents {rest}"))
        .collect();
    let Some(promised) = promised else {
        return first;
    };
    let Some(second) = promised_grant(promised, inner) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for text in with_effects(value, std::slice::from_ref(&second)) {
        let Some(rest) = text.strip_prefix("Permanents ") else {
            continue;
        };
        let also = rest.replacen(" gain ", " also gain ", 1);
        for head in &first {
            found.push(format!(
                "{head} If the gift was promised, permanents {also}"
            ));
        }
    }
    found
}

/// The grant under "If the gift was promised": a `Continuous` of the same group and duration.
fn promised_grant(step: &Value, first: &Value) -> Option<Value> {
    let inner = step.get("If")?;
    if inner.get("condition") != Some(&json!("AdditionalCostPaid"))
        || inner
            .get("otherwise")
            .is_some_and(|other| other != &json!([]))
    {
        return None;
    }
    let [then] = inner.get("then")?.as_array()?.as_slice() else {
        return None;
    };
    let second = grant(then)?;
    (second.get("affected") == first.get("affected")).then(|| then.clone())
}
