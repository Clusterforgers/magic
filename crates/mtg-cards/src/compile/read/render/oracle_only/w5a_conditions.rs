//! Intervening "if" clauses of a trigger the other writers do not word: "if that player controls more lands than you"
//! (the controller of the triggering object against you) and "if another Human entered the battlefield under your
//! control this turn". The ability is written without the condition and the condition is put after its event.

use serde_json::{Value, json};

use super::super::super::selectors::{Form, phrase_text};
use super::super::super::triggers::{pattern_text, subject_text};
use super::w4b_stand::{self, json as to_json, swapped};
use crate::ability::Ability;
use crate::refs::Selector;

/// Adds the wordings of a triggered ability whose intervening "if" only this writer words.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when,
        intervening_if: Some(condition),
        ..
    } = ability
    else {
        return;
    };
    let Some(compare) = to_json(condition).get("Compare").cloned() else {
        return;
    };
    let Some(said) = more_than_you(&compare).or_else(|| entered(&compare)) else {
        return;
    };
    let Some(head) = pattern_text(when) else {
        return;
    };
    let Some(standing) = w4b_stand::stand(ability, |value| {
        value.get_mut("Triggered")?.as_object_mut()?.remove("if");
        Some(())
    }) else {
        return;
    };
    let head = format!("{head},");
    texts.extend(swapped(
        &standing,
        &[(&head, &format!("{head} if {said},"))],
    ));
}

/// The objects a count counts, without the predicates `skip` names.
fn counted(count: &Value, skip: &[Value]) -> Option<Selector> {
    let preds: Vec<Value> = count
        .get("Count")?
        .as_array()?
        .iter()
        .filter(|pred| !skip.contains(pred))
        .cloned()
        .collect();
    serde_json::from_value(Value::Array(preds)).ok()
}

/// "that player controls more lands than you": the objects the triggering object's controller controls against yours.
fn more_than_you(compare: &Value) -> Option<String> {
    let theirs = json!({"ControlledBy": {"ControllerOf": "Triggering"}});
    let yours = json!({"ControlledBy": "You"});
    let (left, right) = (compare.get("left")?, compare.get("right")?);
    let mut left_preds = left.get("Count")?.as_array()?.clone();
    let mut right_preds = right.get("Count")?.as_array()?.clone();
    let at_left = left_preds.iter().position(|pred| *pred == theirs)?;
    let at_right = right_preds.iter().position(|pred| *pred == yours)?;
    left_preds.remove(at_left);
    right_preds.remove(at_right);
    (left_preds == right_preds).then_some(())?;
    let more = match compare.get("op")?.as_str()? {
        "Gt" => "more",
        "Lt" => "fewer",
        _ => return None,
    };
    let what = counted(left, &[theirs])?;
    Some(format!(
        "that player controls {more} {} than you",
        phrase_text(&what, Form::All)?
    ))
}

/// "another Human entered the battlefield under your control this turn": one or more of your permanents entered.
fn entered(compare: &Value) -> Option<String> {
    let entered = json!("EnteredThisTurn");
    let yours = json!({"ControlledBy": "You"});
    let left = compare.get("left")?;
    let preds = left.get("Count")?.as_array()?;
    let asks = compare.get("op")? == "Ge" && compare.get("right")? == 1;
    (asks && preds.contains(&entered) && preds.contains(&yours)).then_some(())?;
    let what = counted(left, &[entered, yours])?;
    Some(format!(
        "{} entered the battlefield under your control this turn",
        subject_text(&what)?
    ))
}
