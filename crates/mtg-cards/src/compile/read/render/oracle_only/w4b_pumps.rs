//! Pumps whose group or amount the renderers do not word: "~ and Drakes you control each get +X/+X" and "+X/+X until
//! end of turn, where X is the amount of life you gained this turn". The ability is written with creatures you control
//! or the cards in your hand in their place and the words are swapped; a pump after another effect may open with
//! "Then".

use super::super::written_variants;
use super::w4b_grants::replace;
use super::w4b_stand::{json, stand, swapped_in};
use crate::ability::Ability;
use serde_json::{Value, json};

/// Adds the wordings of an ability with one of the groups or amounts.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let value = json(ability);
    let creatures = json!({"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]});
    let hand = json!({"Count": [{"In": "Hand"}, {"OwnedBy": "You"}]});
    if let Some(subtype) = with_this(&value) {
        let group =
            json!({"Each": [{"AnyOf": ["This", {"Subtype": subtype}]}, {"ControlledBy": "You"}]});
        let named = format!("~ and {subtype}s you control each get");
        let swaps = [
            ("Creatures you control get", named.as_str()),
            ("creatures you control get", named.as_str()),
        ];
        texts.extend(swap_all(ability, &value, &group, &creatures, &swaps));
    }
    let gained = json!({"Tallied": {"who": "You", "what": "LifeGainedTotal"}});
    let swaps = [(
        "where X is the number of cards in your hand",
        "where X is the amount of life you gained this turn",
    )];
    let written = swap_all(ability, &value, &gained, &hand, &swaps);
    let then: Vec<String> = written.iter().filter_map(|text| then_form(text)).collect();
    texts.extend(written);
    texts.extend(then);
}

/// The subtype of a group of "~ and the creatures of a subtype you control".
fn with_this(value: &Value) -> Option<String> {
    let found = value
        .get("Each")
        .and_then(Value::as_array)
        .and_then(|each| match each.as_slice() {
            [any, control] if *control == json!({"ControlledBy": "You"}) => {
                match any.get("AnyOf")?.as_array()?.as_slice() {
                    [this, subtype] if this == "This" => {
                        Some(subtype.get("Subtype")?.as_str()?.to_owned())
                    }
                    _ => None,
                }
            }
            _ => None,
        });
    found.or_else(|| match value {
        Value::Object(map) => map.values().find_map(with_this),
        Value::Array(items) => items.iter().find_map(with_this),
        _ => None,
    })
}

/// The wordings of the ability with every `from` in its JSON replaced by `to` and the words swapped by each of
/// `swaps` in turn. Nothing when the ability has `to` already or no `from`.
fn swap_all(
    ability: &Ability,
    value: &Value,
    from: &Value,
    to: &Value,
    swaps: &[(&str, &str)],
) -> Vec<String> {
    if replace(&mut value.clone(), to, to).is_some() {
        return Vec::new();
    }
    let Some(written) = stand(ability, |value| {
        replace(value, from, to)?;
        while replace(value, from, to).is_some() {}
        Some(())
    }) else {
        return Vec::new();
    };
    let texts = written_variants(&written);
    swaps
        .iter()
        .flat_map(|swap| swapped_in(&texts, &[*swap]))
        .collect()
}

/// The text with "Then" opening its last sentence, which follows another effect.
fn then_form(text: &str) -> Option<String> {
    let at = text.rfind(". ")?;
    let (head, last) = text.split_at(at + 2);
    let mut chars = last.chars();
    let first = chars.next()?;
    (first.is_uppercase() && !last.starts_with("Then ")).then_some(())?;
    Some(format!(
        "{head}Then {}{}",
        first.to_lowercase(),
        chars.as_str()
    ))
}
