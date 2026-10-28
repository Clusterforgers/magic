//! Returns to the battlefield after an exile or a death, worded after the head the renderers write: a may-exile of the
//! one target followed by its return ("you may exile target creature you control, then return that card to the
//! battlefield under your control") and the return of the dead card at the next end step ("return that card to the
//! battlefield under its owner's control at the beginning of the next end step").

use super::super::alternatives;
use super::standin::with_effects;
use crate::ability::Ability;
use crate::effect::Effect;
use serde_json::{Value, json};

/// Adds the wordings of an ability whose effects are one of the two returns.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(Value::Array(effects)) = serde_json::to_value(ability.effects()) else {
        return;
    };
    let sentence = match effects.as_slice() {
        [effect] => c9_wr_flicker(ability, effect).or_else(|| c9_wr_delayed(effect)),
        _ => None,
    };
    if let Some(sentence) = sentence {
        texts.extend(with_effects(ability, &sentence));
    }
}

/// The owner of the returned card's control, as the data names it: "your" for `You`, "its owner's" for none.
fn c9_wr_under(to: &Value) -> Option<&'static str> {
    let battlefield = to.get("Battlefield")?.as_object()?;
    match (battlefield.len(), battlefield.get("under")) {
        (0, None) => Some("its owner's"),
        (1, Some(Value::String(you))) if you == "You" => Some("your"),
        _ => None,
    }
}

/// "you may exile <the target>, then return that card to the battlefield under <whose> control".
fn c9_wr_flicker(ability: &Ability, effect: &Value) -> Option<String> {
    let [exile, moved] = effect.get("May")?.get("effects")?.as_array()?.as_slice() else {
        return None;
    };
    let exile = exile.get("Exile")?;
    let bound = exile.get("as")?.as_str()?;
    if exile.as_object()?.len() != 2 || exile.get("what")? != &json!({"Target": 0}) {
        return None;
    }
    let moved = moved.get("Move")?;
    if moved.as_object()?.len() != 2 || moved.get("what")? != &json!({"Bound": bound}) {
        return None;
    }
    let under = c9_wr_under(moved.get("to")?)?;
    let plain: Effect = serde_json::from_value(json!({"Exile": {"what": {"Target": 0}}})).ok()?;
    let exiled = alternatives(ability.targets(), &[plain])
        .into_iter()
        .find_map(|sentences| match sentences.as_slice() {
            [one] => Some(one.clone()),
            _ => None,
        })
        .filter(|sentence| sentence.starts_with("exile "))?;
    Some(format!(
        "you may {exiled}, then return that card to the battlefield under {under} control"
    ))
}

/// "return that card to the battlefield under <whose> control at the beginning of the next end step".
fn c9_wr_delayed(effect: &Value) -> Option<String> {
    let delayed = effect.get("DelayedTrigger")?;
    if delayed.as_object()?.len() != 2
        || delayed.get("when")? != &json!({"StepBegins": {"step": "End"}})
    {
        return None;
    }
    let [moved] = delayed.get("effects")?.as_array()?.as_slice() else {
        return None;
    };
    let moved = moved.get("Move")?;
    if moved.as_object()?.len() != 2 || moved.get("what")? != "Triggering" {
        return None;
    }
    let under = c9_wr_under(moved.get("to")?)?;
    Some(format!(
        "return that card to the battlefield under {under} control at the beginning of the next end step"
    ))
}
