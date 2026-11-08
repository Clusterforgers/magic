//! A card exiled from a library that its controller may put onto the battlefield when it is of a kind, with a reward when
//! they do not: "Whenever equipped creature deals combat damage to a player, exile the top card of that player's
//! library. If it's a permanent card, you may put it onto the battlefield under your control. If you don't, create a 1/1
//! blue Fish creature token." The exile and the reward are worded as triggers of their own and joined by the two
//! sentences between them.

use serde_json::{Value, json};

use super::super::super::r6d_kind::all_texts;
use super::super::written_variants;
use crate::ability::Ability;
use crate::value::Condition;

/// The steps of the trigger once it is known to have the shape: the exile, the condition on the card's kind and the
/// reward.
struct Shape {
    exile: Value,
    kind: Condition,
    reward: Vec<Value>,
}

/// Adds the wordings of a trigger that exiles a card, may put it onto the battlefield when it is of a kind, and rewards
/// the player when they do not.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(shape) = shape(&value) else {
        return;
    };
    let mut kinds: Vec<String> = all_texts(&shape.kind)
        .into_iter()
        .filter(|text| text.starts_with("it's "))
        .collect();
    // The card of a permanent kind is a permanent card.
    let cards: Vec<String> = kinds
        .iter()
        .filter(|text| text.ends_with(" permanent"))
        .map(|text| format!("{text} card"))
        .collect();
    kinds.extend(cards);
    let (Some(exiled), Some(rewarded)) = (
        twin(&value, vec![shape.exile.clone()]),
        twin(&value, shape.reward.clone()),
    ) else {
        return;
    };
    for exile in &exiled {
        let Some(at) = exile.find(", exile ") else {
            continue;
        };
        let head = &exile[..at + 2];
        for reward in rewarded.iter().filter_map(|text| text.strip_prefix(head)) {
            for kind in &kinds {
                let text = format!(
                    "{exile} If {kind}, you may put it onto the battlefield under your control. If you don't, {reward}"
                );
                if !texts.contains(&text) {
                    texts.push(text);
                }
            }
        }
    }
}

// The kind is worded as for one target ("it's a permanent card"); the card is the only object the sentence names.
/// The three steps of the trigger when they are the exile, the optional move on the kind and the reward for not moving.
fn shape(value: &Value) -> Option<Shape> {
    let effects = value.get("Triggered")?.get("effects")?.as_array()?;
    let [exile, kind, reward] = effects.as_slice() else {
        return None;
    };
    let name = exile.pointer("/ExileTop/as")?.as_str()?;
    let bound = |name: &str| json!({ "Bound": name });
    let counted = kind.pointer("/If/condition/CountOf")?;
    let selector = counted.get("selector")?;
    let [may] = kind.pointer("/If/then")?.as_array()?.as_slice() else {
        return None;
    };
    let [step] = may.pointer("/May/effects")?.as_array()?.as_slice() else {
        return None;
    };
    let moved = step.pointer("/Move/as")?.as_str()?;
    let asked = reward.pointer("/If/condition/CountOf")?;
    let exact = counted.get("subject") == Some(&bound(name))
        && counted.get("op") == Some(&json!("Ge"))
        && counted.get("right") == Some(&json!(1))
        && selector.as_array().is_some_and(|list| !list.is_empty())
        && kind.pointer("/If/otherwise").is_none_or(is_empty)
        && may.pointer("/May/who").is_none_or(|who| who == "You")
        && step.pointer("/Move/what") == Some(&bound(name))
        && step.pointer("/Move/to") == Some(&json!({"Battlefield": {"under": "You"}}))
        && asked.get("subject") == Some(&bound(moved))
        && asked.get("op") == Some(&json!("Eq"))
        && asked.get("right") == Some(&json!(0))
        && asked.get("selector") == Some(&json!([]))
        && reward.pointer("/If/otherwise").is_none_or(is_empty);
    exact.then_some(())?;
    Some(Shape {
        exile: exile.clone(),
        kind: serde_json::from_value(json!({"CountOf": {
            "subject": {"Target": 0}, "selector": selector, "op": "Ge", "right": 1
        }}))
        .ok()?,
        reward: reward.pointer("/If/then")?.as_array()?.clone(),
    })
}

/// Whether the value is an empty list.
fn is_empty(value: &Value) -> bool {
    value.as_array().is_some_and(Vec::is_empty)
}

/// The wordings of the trigger with only these steps.
fn twin(value: &Value, steps: Vec<Value>) -> Option<Vec<String>> {
    let mut twin = value.clone();
    twin["Triggered"]["effects"] = Value::Array(steps);
    let twin: Ability = serde_json::from_value(twin).ok()?;
    Some(written_variants(&twin))
}
