//! The snow supertype in a phrase ("Untap another target snow permanent.", "target snow land"): the ability is written
//! with basic in its place, the supertype the selectors word, and the word is swapped back.

use serde_json::json;

use super::w4d_parts::{swapped, wordings};
use crate::ability::Ability;

/// Adds the wordings of an ability that names the snow supertype and not the basic one.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let basic = json!({"Supertype": "Basic"});
    if swapped(&mut value.clone(), &basic, &basic) {
        return;
    }
    if !swapped(&mut value, &json!({"Supertype": "Snow"}), &basic) {
        return;
    }
    texts.extend(wordings(value).iter().filter_map(|text| w8c_snowed(text)));
}

/// The wording with each word "basic" (also "nonbasic") read as "snow"; `None` when it has none.
fn w8c_snowed(text: &str) -> Option<String> {
    let mut any = false;
    let words: Vec<String> = text
        .split(' ')
        .map(|word| {
            let swapped = word.replacen("basic", "snow", 1);
            let plain = word.trim_start_matches("non");
            if plain.starts_with("basic") {
                any = true;
                swapped
            } else {
                word.to_owned()
            }
        })
        .collect();
    any.then(|| words.join(" "))
}
