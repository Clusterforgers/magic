//! Protection with an exception, written back: the ability with plain protection from the same quality, then "This
//! effect doesn't remove ~.", "This effect doesn't remove Auras." or "This effect doesn't remove Auras and Equipment you
//! control that are already attached to it.". Any other exception has no words.

use serde_json::Value;

use crate::ability::Ability;

/// The ability with its one protection with an exception made plain, and the sentence of the exception.
pub fn e14d_plain(ability: &Ability) -> Option<(Ability, &'static str)> {
    let mut value = serde_json::to_value(ability).ok()?;
    let mut found = Vec::new();
    plain_in(&mut value, &mut found);
    let [spares] = found.as_slice() else {
        return None;
    };
    let sentence = sentence(spares)?;
    Some((serde_json::from_value(value).ok()?, sentence))
}

/// Replaces each protection with an exception by plain protection, and lists the exceptions.
fn plain_in(value: &mut Value, found: &mut Vec<Value>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::Object(inner)) = map.get("ProtectionSparing")
                && let (Some(from), Some(spares)) = (inner.get("from"), inner.get("spares"))
            {
                let (from, spares) = (from.clone(), spares.clone());
                map.remove("ProtectionSparing");
                map.insert("Protection".to_owned(), from);
                found.push(spares);
                return;
            }
            map.values_mut().for_each(|inner| plain_in(inner, found));
        }
        Value::Array(items) => items.iter_mut().for_each(|inner| plain_in(inner, found)),
        _ => {}
    }
}

/// The words of an exception the translator writes.
fn sentence(spares: &Value) -> Option<&'static str> {
    let known: [(&str, &str); 3] = [
        (r#"["This"]"#, "This effect doesn't remove ~."),
        (
            r#"[{"Subtype":"Aura"}]"#,
            "This effect doesn't remove Auras.",
        ),
        (
            r#"[{"AnyOf":[{"Subtype":"Aura"},{"Subtype":"Equipment"}]},{"ControlledBy":"You"}]"#,
            "This effect doesn't remove Auras and Equipment you control that are already attached to it.",
        ),
    ];
    known.into_iter().find_map(|(json, words)| {
        (serde_json::from_str::<Value>(json).ok().as_ref() == Some(spares)).then_some(words)
    })
}
