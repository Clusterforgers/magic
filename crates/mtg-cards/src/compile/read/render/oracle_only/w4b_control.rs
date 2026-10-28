//! A permanent given to a chosen opponent ("When this creature enters, an opponent gains control of it."): the ability
//! is written with a target opponent in place of the choice, and "target opponent gains control of ~" becomes "an
//! opponent gains control of it".

use super::w4b_stand::{json, stand, swapped};
use crate::ability::Ability;
use serde_json::{Value, json};

/// Adds the wordings of an ability whose first effect chooses an opponent who then gains control of the card.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let value = json(ability);
    let Some(body) = value.as_object().and_then(|map| map.values().next()) else {
        return;
    };
    let Some([choose, given]) = body
        .get("effects")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .and_then(|effects| <&[Value; 2]>::try_from(effects).ok())
    else {
        return;
    };
    let Some(name) = choose
        .pointer("/Choose")
        .filter(|choose| choose.get("from") == Some(&json!("Opponents")))
        .and_then(|choose| choose.get("as"))
    else {
        return;
    };
    let shape = json!({"Continuous": {"affected": "This", "mods": [{"SetController": {"Bound": name}}], "duration": "Forever"}});
    if *given != shape || body.get("targets").is_some() {
        return;
    }
    let Some(written) = stand(ability, |value| {
        let body = value
            .as_object_mut()?
            .values_mut()
            .next()?
            .as_object_mut()?;
        body.insert("targets".to_owned(), json!([{"what": "Opponent"}]));
        body.insert(
            "effects".to_owned(),
            json!([{"Continuous": {"affected": "This", "mods": [{"SetController": {"Target": 0}}], "duration": "Forever"}}]),
        );
        Some(())
    }) else {
        return;
    };
    texts.extend(swapped(
        &written,
        &[(
            "target opponent gains control of ~",
            "an opponent gains control of it",
        )],
    ));
}
