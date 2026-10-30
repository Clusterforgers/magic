//! A target an effect uses and the ability does not declare is declared from the printed "target ..." phrases, read by
//! the reader the compiler uses for printed text, when the phrases are as many as the targets the effects use.

use serde_json::Value;

use super::targets::{scoped, used_by};
use super::text::ability_text;
use super::{EFFECT_ABILITIES, Repair, RepairClass, Site, body_of};
use crate::compile::target_phrases;

/// Declares the targets the effects of the ability use beyond the ones it declares.
pub(super) fn repair(ability: &mut Value, site: &Site<'_>, repairs: &mut Vec<Repair>) {
    let Some((name, body)) = body_of(ability) else {
        return;
    };
    if !EFFECT_ABILITIES.contains(&name) || body.values().any(scoped) {
        return;
    }
    let uses = used_by(body);
    let Some(&last) = uses.last() else {
        return;
    };
    let needed = usize::try_from(last)
        .unwrap_or(usize::MAX)
        .saturating_add(1);
    let declared: Vec<Value> = body
        .get("targets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // The uses must be 0..needed, or declaring them leaves one that no effect takes.
    if uses.len() != needed || declared.len() >= needed {
        return;
    }
    let Some(text) = ability_text(site.face, name, site.nth, site.count) else {
        return;
    };
    let phrases: Vec<Value> = target_phrases(&text)
        .iter()
        .filter_map(|spec| serde_json::to_value(spec).ok())
        .collect();
    // What the file declares stays as it is, so it must be what the text says first.
    if phrases.len() != needed || phrases.get(..declared.len()) != Some(&declared[..]) {
        return;
    }
    let added = needed - declared.len();
    body.insert("targets".to_owned(), Value::Array(phrases));
    repairs.push(Repair {
        class: RepairClass::TargetDeclared,
        at: site.at.to_owned(),
        note: format!("the effects use {needed} target(s) and {added} are not declared: declared from the printed phrase(s)"),
    });
}
