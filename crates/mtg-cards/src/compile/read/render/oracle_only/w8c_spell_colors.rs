//! A spell cast of any of several colors, said after the noun: "Whenever you cast a spell that's white, blue, black,
//! or red, put a +1/+1 counter on this creature." The trigger is written with the first color and the phrase swapped.

use serde_json::{Value, json};

use super::w4d_parts::wordings;
use crate::ability::Ability;

/// Adds the wordings of a trigger on a spell cast of one of two or more colors.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(what) = value.pointer_mut("/Triggered/when/SpellCast/what") else {
        return;
    };
    let Some(colors) = w8c_colors(what) else {
        return;
    };
    let Some(first) = colors.first().cloned() else {
        return;
    };
    *what = json!([{"Color": first}]);
    let plain = format!("a {} spell", first.to_lowercase());
    let listed = format!("a spell that's {}", w8c_listed(&colors));
    for text in wordings(value) {
        if text.matches(&plain).count() == 1 {
            texts.push(text.replacen(&plain, &listed, 1));
        }
    }
}

/// The colors of a selector that is any one of two or more colors and nothing else.
fn w8c_colors(what: &Value) -> Option<Vec<String>> {
    let [any] = what.as_array()?.as_slice() else {
        return None;
    };
    let options = any.get("AnyOf")?.as_array()?;
    let colors: Option<Vec<String>> = options
        .iter()
        .map(|option| option.get("Color")?.as_str().map(str::to_owned))
        .collect();
    colors.filter(|colors| colors.len() >= 2)
}

/// The colors in lower case as a list with "or": "white or blue", "white, blue, black, or red".
fn w8c_listed(colors: &[String]) -> String {
    let lowered: Vec<String> = colors.iter().map(|color| color.to_lowercase()).collect();
    match lowered.as_slice() {
        [one, two] => format!("{one} or {two}"),
        [rest @ .., last] => format!("{}, or {last}", rest.join(", ")),
        [] => String::new(),
    }
}
