//! A group of creatures with a name: "Creatures named Rock Lobster can't attack or block." The group is written with a
//! subtype no card has in place of the name ("W13amark creatures") and the words are swapped.

use serde_json::{Value, json};

use super::w4d_parts::{capitalized, wordings};
use crate::ability::Ability;

/// The subtype that stands for the name while the ability is written.
const W13A_MARK: &str = "W13amark";

/// Adds the wordings of an ability with one group of creatures named by another card's name.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let mut names = Vec::new();
    w13a_marked(&mut value, &mut names);
    let [name] = names.as_slice() else {
        return;
    };
    let plural = format!("{W13A_MARK} creatures");
    for text in wordings(value) {
        if text.matches(W13A_MARK).count() != 1 || text.contains(&format!("{plural} you")) {
            continue;
        }
        let said = text.replace(&plural, &format!("creatures named {name}"));
        if !said.contains(W13A_MARK) {
            texts.push(capitalized(&said));
        }
    }
}

/// Swaps the name of each group of creatures for the mark, collecting the names.
fn w13a_marked(value: &mut Value, names: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::Array(preds)) = map.get_mut("Each")
                && preds.contains(&json!({"Type": "Creature"}))
            {
                for pred in preds.iter_mut() {
                    if let Some(name) = pred.get("Named").and_then(Value::as_str) {
                        names.push(name.to_owned());
                        *pred = json!({"Subtype": W13A_MARK});
                    }
                }
            }
            map.values_mut().for_each(|inner| w13a_marked(inner, names));
        }
        Value::Array(items) => items.iter_mut().for_each(|inner| w13a_marked(inner, names)),
        _ => {}
    }
}
