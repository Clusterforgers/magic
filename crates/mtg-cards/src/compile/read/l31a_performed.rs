//! The logs of keyword actions (`Effect::Performed`) the readers add after the steps of an investigation, a discover and
//! a manifest dread, and the writers' view of them: an investigation is written as the Clue tokens it makes, so its log
//! is dropped before the ability is worded when it follows those tokens by the same player as many times.

use serde_json::Value;

use crate::ability::Ability;
use crate::did::Action;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::Amount;

/// The log of `who` investigating `times` times.
pub fn investigated(who: Ref, times: Amount) -> Effect {
    Effect::Performed {
        action: Action::Investigated,
        who,
        times,
        card: None,
        value: None,
        to: None,
    }
}

/// The steps of an investigation: the Clue token `create` makes, then its log by the same player.
pub fn investigation(create: Effect) -> Vec<Effect> {
    let Effect::CreateToken { who, count, .. } = &create else {
        return vec![create];
    };
    let log = investigated(who.clone(), count.clone());
    vec![create, log]
}

/// Whether `step` makes Clue tokens for `who`, `times` of them.
fn clues_for(step: &Value, who: &Value, times: &Value) -> bool {
    let Some(made) = step.get("CreateToken") else {
        return false;
    };
    let clue = made.get("token") == Some(&serde_json::json!({"Named": "clue"}));
    let maker = made.get("who").unwrap_or(&Value::Null);
    let count = made.get("count").unwrap_or(&Value::Null);
    clue && maker == who && count == times
}

/// Drops from every list of steps each investigation log that follows the Clue tokens it counts.
fn drop_logs(value: &mut Value) -> bool {
    let mut dropped = false;
    match value {
        Value::Array(steps) => {
            let mut at = 1;
            while at < steps.len() {
                let log = steps[at]
                    .get("Performed")
                    .filter(|log| log.get("action") == Some(&Value::from("Investigated")));
                let follows = log.is_some_and(|log| {
                    let who = log.get("who").unwrap_or(&Value::Null);
                    let times = log.get("times").unwrap_or(&Value::Null);
                    clues_for(&steps[at - 1], who, times)
                });
                if follows {
                    steps.remove(at);
                    dropped = true;
                } else {
                    at += 1;
                }
            }
            for step in steps.iter_mut() {
                dropped |= drop_logs(step);
            }
        }
        Value::Object(fields) => {
            for field in fields.values_mut() {
                dropped |= drop_logs(field);
            }
        }
        _ => {}
    }
    dropped
}

/// The ability as the writers word it when it logs an investigation after its Clue tokens: without those logs.
pub(super) fn l31a_unlogged(ability: &Ability) -> Option<Ability> {
    let mut value = serde_json::to_value(ability).ok()?;
    drop_logs(&mut value).then_some(())?;
    serde_json::from_value(value).ok()
}

/// Whether the steps are one investigation: its Clue tokens and their log, one choice for a "may".
pub fn is_investigation(steps: &[Effect]) -> bool {
    match steps {
        [create @ Effect::CreateToken { .. }, _] => investigation(create.clone()) == steps,
        _ => false,
    }
}
