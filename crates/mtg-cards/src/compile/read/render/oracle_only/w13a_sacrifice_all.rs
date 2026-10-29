//! A sacrifice of a whole group: "When ~ leaves the battlefield, sacrifice all non-Ogre creatures you control." The
//! rules sacrifice as many as the group counts; the sacrifice is written for one ("sacrifice a non-Ogre creature you
//! control") and the words are put in the plural.

use serde_json::Value;

use super::w4d_parts::{parts, with_effects};
use crate::ability::Ability;

/// Adds the wordings of an ability whose one effect sacrifices as many creatures of a filter as the filter counts.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((value, mut effects)) = parts(ability) else {
        return;
    };
    let [effect] = effects.as_mut_slice() else {
        return;
    };
    let Some(sacrifice) = effect.get_mut("Sacrifice").and_then(Value::as_object_mut) else {
        return;
    };
    let filter = sacrifice.get("filter").cloned();
    let all = sacrifice
        .get("count")
        .and_then(|count| count.get("Count"))
        .is_some_and(|counted| Some(counted) == filter.as_ref());
    if !all || sacrifice.contains_key("who") {
        return;
    }
    sacrifice.remove("count");
    for text in with_effects(&value, &effects) {
        if let Some(said) = w13a_plural(&text) {
            texts.push(said);
        }
    }
}

/// "sacrifice a non-Ogre creature you control" as "sacrifice all non-Ogre creatures you control".
fn w13a_plural(text: &str) -> Option<String> {
    let (lead, rest) = text.split_once("acrifice a ")?;
    if lead.ends_with("you s") {
        return None;
    }
    let (kind, tail) = rest.split_once("creature ")?;
    if kind.contains(' ') && !kind.ends_with(' ') {
        return None;
    }
    Some(format!("{lead}acrifice all {kind}creatures {tail}"))
}
