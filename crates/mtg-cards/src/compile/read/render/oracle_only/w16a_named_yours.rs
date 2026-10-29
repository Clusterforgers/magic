//! Creatures you control with a name: "Each creature you control named Pack Mastiff gets +1/+0 until end of turn." The
//! name is written as a subtype no card has ("each W16aname creature you control") and the words are swapped.

use serde_json::{Value, json};

use super::w4d_parts::{capitalized, wordings};
use crate::ability::Ability;

/// The subtype that stands for the name while the ability is written.
const W16A_NAME: &str = "W16aname";

/// Adds the wordings of an ability with one group of creatures you control that have a name.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let mut names = Vec::new();
    w16a_marked(&mut value, &mut names);
    let [name] = names.as_slice() else {
        return;
    };
    let swaps = [
        (
            format!("each {W16A_NAME} creature you control gets"),
            format!("each creature you control named {name} gets"),
        ),
        (
            format!("{W16A_NAME} creatures you control get"),
            format!("creatures you control named {name} get"),
        ),
    ];
    for text in wordings(value) {
        if text.matches(W16A_NAME).count() != 1 {
            continue;
        }
        for (from, to) in &swaps {
            for (from, to) in [
                (from.clone(), to.clone()),
                (capitalized(from), capitalized(to)),
            ] {
                if text.contains(&from) {
                    texts.push(text.replace(&from, &to));
                }
            }
        }
    }
}

/// Swaps the name of each group of creatures you control for the mark, collecting the names.
fn w16a_marked(value: &mut Value, names: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::Array(preds)) = map.get_mut("Each")
                && preds.contains(&json!({"Type": "Creature"}))
                && preds.contains(&json!({"ControlledBy": "You"}))
            {
                for pred in preds.iter_mut() {
                    if let Some(name) = pred.get("Named").and_then(Value::as_str) {
                        names.push(name.to_owned());
                        *pred = json!({"Subtype": W16A_NAME});
                    }
                }
            }
            map.values_mut().for_each(|inner| w16a_marked(inner, names));
        }
        Value::Array(items) => items.iter_mut().for_each(|inner| w16a_marked(inner, names)),
        _ => {}
    }
}
