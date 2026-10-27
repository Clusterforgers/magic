//! A group of up to X targets that each get the same: "Put a +1/+1 counter on each of up to X target creatures. Those
//! creatures gain flying until end of turn." The renderers word one target ("on target creature", "It gains").

use serde_json::json;

use super::super::variants;
use super::twin_json::{inner_mut, twin};
use crate::ability::Ability;

/// Adds the wordings of an ability whose one target specification is "up to X" targets, from the wordings of the same
/// ability with one target.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some(plain) = twin(ability, |value| {
        let specs = inner_mut(value)?.get_mut("targets")?.as_array_mut()?;
        let [spec] = specs.as_mut_slice() else {
            return None;
        };
        (spec.get("bounds") == Some(&json!({"min": 0, "max": "X"}))).then_some(())?;
        spec.as_object_mut()?.remove("bounds").map(|_| ())
    }) else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter(|text| text.contains(" on target creature"))
        .map(|text| {
            text.replacen(
                " on target creature",
                " on each of up to X target creatures",
                1,
            )
            .replace(". It gains ", ". Those creatures gain ")
            .replace(". It gets ", ". Those creatures get ")
        })
        .collect();
    texts.extend(said);
}
