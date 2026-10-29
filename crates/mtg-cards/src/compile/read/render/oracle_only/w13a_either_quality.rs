//! A target with one of two qualities: "target creature or planeswalker that's green or white", "target creature with
//! power or toughness 4 or greater". The target is written with a comparison of power in place of the choice and the
//! words of that comparison are swapped for the two qualities.

use serde_json::{Value, json};

use super::w4d_parts::wordings;
use crate::ability::Ability;

/// A comparison no card names, written as "with power 99 or greater", that stands for the two colors.
const W13A_MARK: u64 = 99;

/// Adds the wordings of an ability with one target whose selector holds a choice of two colors or two sizes.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(preds) = w13a_target_preds(&mut value) else {
        return;
    };
    let mut said = None;
    for pred in preds.iter_mut() {
        let Some([first, second]) = pred
            .get("AnyOf")
            .and_then(Value::as_array)
            .and_then(|options| <&[Value; 2]>::try_from(options.as_slice()).ok())
            .cloned()
        else {
            continue;
        };
        if said.is_some() {
            return;
        }
        if let (Some(one), Some(other)) = (w13a_color(&first), w13a_color(&second)) {
            *pred = json!({"Compare": {"op": "Ge", "stat": "Power", "to": W13A_MARK}});
            said = Some((
                format!("with power {W13A_MARK} or greater"),
                format!("that's {one} or {other}"),
            ));
        } else if let Some(size) = w13a_sizes(&first, &second) {
            *pred = first;
            said = Some(size);
        }
    }
    let Some((from, to)) = said else {
        return;
    };
    for text in wordings(value) {
        if text.matches(&from).count() == 1 {
            texts.push(text.replace(&from, &to));
        }
    }
}

/// The predicates of the one object target of the ability.
fn w13a_target_preds(value: &mut Value) -> Option<&mut Vec<Value>> {
    let inner = value.as_object_mut()?.values_mut().next()?;
    match inner.get_mut("targets")?.as_array_mut()?.as_mut_slice() {
        [target] => target.get_mut("what")?.get_mut("Object")?.as_array_mut(),
        _ => None,
    }
}

/// The word of a color predicate.
fn w13a_color(pred: &Value) -> Option<String> {
    pred.get("Color")?.as_str().map(str::to_lowercase)
}

/// For two comparisons of power and toughness with the same operator and number: the words of the first ("with power
/// 4 ") and of both ("with power or toughness 4 ").
fn w13a_sizes(first: &Value, second: &Value) -> Option<(String, String)> {
    let one = first.get("Compare")?;
    let other = second.get("Compare")?;
    let to = one.get("to")?.as_u64()?;
    let same = one.get("op") == other.get("op") && one.get("to") == other.get("to");
    let stats = (one.get("stat")?.as_str()?, other.get("stat")?.as_str()?);
    if !same || stats != ("Power", "Toughness") || to == W13A_MARK {
        return None;
    }
    Some((
        format!("with power {to} "),
        format!("with power or toughness {to} "),
    ))
}
