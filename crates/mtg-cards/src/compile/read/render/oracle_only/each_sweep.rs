//! A sweep says "each" with a singular noun where the renderers write "all" and a plural: "Destroy each nonland
//! permanent with mana value X or less." A creature "that isn't all colors" is a comparison of its colors with five.

use serde_json::Value;

use super::super::{Form, phrase_text, variants};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, Cmp, StatKind};

/// The verbs whose sweep a card words with "each".
const VERBS: [&str; 2] = ["destroy", "exile"];

/// Adds the wordings of a destroy or an exile of every object a selector names, with "each".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for effect in ability.effects() {
        let (verb, selector) = match effect {
            Effect::Destroy {
                what: Ref::Each(selector),
                ..
            } => (VERBS[0], selector),
            Effect::Exile {
                what: Ref::Each(selector),
                ..
            } => (VERBS[1], selector),
            _ => continue,
        };
        let (plain, suffix) = split_colors(selector);
        let (Some(all), Some(each)) = (
            phrase_text(&plain, Form::All),
            phrase_text(&plain, Form::Each),
        ) else {
            continue;
        };
        let source = if suffix.is_empty() {
            texts.clone()
        } else {
            let Some(without) = replaced(ability, selector, &plain) else {
                continue;
            };
            variants(&without)
        };
        for (said, named) in [
            (
                format!("{verb} all {all}"),
                format!("{verb} each {each}{suffix}"),
            ),
            (
                format!("{} all {all}", capital(verb)),
                format!("{} each {each}{suffix}", capital(verb)),
            ),
        ] {
            texts.extend(
                source
                    .iter()
                    .filter(|text| text.contains(&said))
                    .map(|text| text.replacen(&said, &named, 1)),
            );
        }
    }
}

/// The verb with its capital.
fn capital(verb: &str) -> String {
    let mut letters = verb.chars();
    letters
        .next()
        .map(|first| first.to_uppercase().chain(letters).collect())
        .unwrap_or_default()
}

/// The selector without the comparison of the number of colors with five, and the words that comparison says.
fn split_colors(selector: &Selector) -> (Selector, &'static str) {
    let all_colors = Pred::Compare {
        stat: StatKind::Colors,
        op: Cmp::Lt,
        to: Amount::Fixed(5),
    };
    if !selector.0.contains(&all_colors) {
        return (selector.clone(), "");
    }
    let rest = selector.0.iter().filter(|pred| **pred != all_colors);
    (Selector(rest.cloned().collect()), " that isn't all colors")
}

/// The ability with `from` in the place of `to`, wherever the data holds that selector.
fn replaced(ability: &Ability, from: &Selector, to: &Selector) -> Option<Ability> {
    let mut json = serde_json::to_value(ability).ok()?;
    let (from, to) = (
        serde_json::to_value(from).ok()?,
        serde_json::to_value(to).ok()?,
    );
    swap(&mut json, &from, &to);
    serde_json::from_value(json).ok()
}

/// Puts `to` in the place of every `from` in the data.
fn swap(value: &mut Value, from: &Value, to: &Value) {
    if value == from {
        value.clone_from(to);
        return;
    }
    match value {
        Value::Array(items) => items.iter_mut().for_each(|item| swap(item, from, to)),
        Value::Object(map) => map.values_mut().for_each(|item| swap(item, from, to)),
        _ => {}
    }
}
