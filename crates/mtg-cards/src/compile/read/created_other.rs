//! "Create X 1/1 white Soldier creature tokens. If X is 5 or more, destroy all other creatures.": in a spell, "other"
//! after a creation names everything but the tokens the spell created (Martial Coup rulings), read as
//! `Not(Is(Bound("tokens")))` with the creation bound; the writer words that shape as its "other" twin.

use serde_json::Value;

use super::token_binds::{TOKENS, bind, tokens};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Pred;

/// "other" as a selector predicate.
fn other() -> Value {
    Value::String("Other".to_owned())
}

/// Not one of the tokens created.
fn not_created() -> Value {
    serde_json::to_value(Pred::Not(Box::new(Pred::Is(tokens())))).unwrap_or(Value::Null)
}

/// Replaces every list item equal to `from` with `to`; true when one was replaced.
fn swap(value: &mut Value, from: &Value, to: &Value) -> bool {
    match value {
        Value::Array(items) => {
            let mut found = false;
            for item in items {
                if item == from {
                    *item = to.clone();
                    found = true;
                } else {
                    found |= swap(item, from, to);
                }
            }
            found
        }
        Value::Object(map) => map
            .values_mut()
            .fold(false, |found, item| swap(item, from, to) | found),
        _ => false,
    }
}

/// How many times the JSON text holds the pattern.
fn count(value: &Value, pattern: &Value) -> usize {
    value.to_string().matches(&pattern.to_string()).count()
}

/// The position of the last creation of the effects.
fn last_creation(effects: &[Effect]) -> Option<usize> {
    effects
        .iter()
        .rposition(|effect| matches!(effect, Effect::CreateToken { .. }))
}

/// Reads "other" in the steps after the last creation of a spell as other than the tokens it created.
pub fn created_other(effects: &mut Vec<Effect>) {
    let Some(at) = last_creation(effects) else {
        return;
    };
    if !matches!(&effects[at], Effect::CreateToken { bind: None, .. }) {
        return;
    }
    let Ok(mut after) = serde_json::to_value(&effects[at + 1..]) else {
        return;
    };
    if !swap(&mut after, &other(), &not_created()) {
        return;
    }
    let Ok(after) = serde_json::from_value::<Vec<Effect>>(after) else {
        return;
    };
    effects.truncate(at + 1);
    effects.extend(after);
    bind(effects);
}

/// The spell with "other" in place of "not the tokens created" and its creation unbound, when the binding is used only
/// that way: the twin the writers word.
pub(super) fn created_other_twin(ability: &Ability) -> Option<Ability> {
    let Ability::Spell { targets, effects } = ability else {
        return None;
    };
    let at = last_creation(effects)?;
    let mut effects = effects.clone();
    let Effect::CreateToken { bind, .. } = &mut effects[at] else {
        return None;
    };
    (bind.as_deref() == Some(TOKENS)).then_some(())?;
    *bind = None;
    let mut after = serde_json::to_value(&effects[at + 1..]).ok()?;
    let named = serde_json::to_value(tokens()).ok()?;
    let uses = count(&after, &named);
    (uses > 0 && uses == count(&after, &not_created())).then_some(())?;
    swap(&mut after, &not_created(), &other());
    effects.truncate(at + 1);
    effects.extend(serde_json::from_value::<Vec<Effect>>(after).ok()?);
    Some(Ability::Spell {
        targets: targets.clone(),
        effects,
    })
}

/// Whether a spell says "other" after a creation with the source meant: it spares nothing, so no wording reads as it.
pub(super) fn other_after_creation(ability: &Ability) -> bool {
    let Ability::Spell { effects, .. } = ability else {
        return false;
    };
    let Some(at) = last_creation(effects) else {
        return false;
    };
    serde_json::to_value(&effects[at + 1..])
        .is_ok_and(|mut after| swap(&mut after, &other(), &other()))
}
