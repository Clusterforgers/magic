//! A discard by the owner of the target the earlier effects named: "Return target permanent to its
//! owner's hand. Then that player discards a card."

use serde_json::Value;

use super::w4d_parts::{parts, with_effects};
use crate::ability::Ability;

/// Adds the wordings of an ability whose last effect is one card discarded by the owner of its target, named after "its owner's".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((value, effects)) = parts(ability) else {
        return;
    };
    let Some((last, before)) = effects.split_last() else {
        return;
    };
    let Some(target) = w8c_discarder(last) else {
        return;
    };
    let named = before.iter().any(|effect| {
        effect
            .as_object()
            .and_then(|map| map.values().next())
            .and_then(|body| body.get("what"))
            == Some(target)
    });
    if !named {
        return;
    }
    for lead in with_effects(&value, before)
        .into_iter()
        .filter(|lead| lead.contains("owner's"))
    {
        for then in ["Then that player", "That player"] {
            texts.push(format!("{lead} {then} discards a card."));
        }
    }
}

/// The target whose owner discards one card of their choice.
fn w8c_discarder(effect: &Value) -> Option<&Value> {
    let discard = effect.get("Discard")?.as_object()?;
    if discard.len() != 1 {
        return None;
    }
    let who = discard.get("who")?;
    let target = who.get("OwnerOf")?;
    target.get("Target")?;
    Some(target)
}
