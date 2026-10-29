//! The source said "it" after a trigger on the source: "Whenever this creature becomes blocked, untap it." The
//! renderers name the source again ("untap ~").

use serde_json::Value;

use crate::ability::Ability;

/// Adds the wordings of a trigger on the source whose effects name the source as an object, with "it" in place of `~`.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(value) = serde_json::to_value(ability) else {
        return;
    };
    if !w12a_on_source(&value) {
        return;
    }
    let extra: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let (event, rest) = text.split_once(", ")?;
            if !event.contains('~') || rest.starts_with('~') || !rest.contains(" ~") {
                return None;
            }
            let said = rest.replace(" ~.", " it.").replace(" ~ ", " it ");
            (said != rest).then(|| format!("{event}, {said}"))
        })
        .collect();
    texts.extend(extra);
}

/// Whether the ability is a trigger on an event of the source alone.
fn w12a_on_source(value: &Value) -> bool {
    value
        .get("Triggered")
        .and_then(|triggered| triggered.get("when"))
        .and_then(Value::as_object)
        .and_then(|when| when.values().next())
        .and_then(|event| event.get("what"))
        .and_then(Value::as_array)
        .is_some_and(|what| what.len() == 1 && what[0] == "This")
}
