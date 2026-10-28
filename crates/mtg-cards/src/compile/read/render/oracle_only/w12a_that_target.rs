//! A later sentence that names the target of the first one by a pronoun: "Tap target creature. This creature deals
//! damage equal to its power to that creature.", "Target creature gets +2/+0 until end of turn. Regenerate it. You lose
//! 2 life.", "~ deals 2 damage to target player. That player discards two cards."

use serde_json::{Value, json};

use super::w4d_parts::{capitalized, parts, with_effects, wordings};
use super::w8c_then_tap::w8c_names;
use crate::ability::Ability;

/// Adds the wordings of an ability with one target whose first effect names it and exactly one later effect names it
/// again, as "it" or "that creature".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((value, effects)) = parts(ability) else {
        return;
    };
    let Some(targets) = w12a_targets(&value) else {
        return;
    };
    let Some(noun) = w12a_noun(&targets[0]) else {
        return;
    };
    let target = json!({"Target": 0});
    let Some((first, tail)) = effects.split_first() else {
        return;
    };
    let again = tail
        .iter()
        .filter(|effect| w8c_names(effect, &target))
        .count();
    if !w8c_names(first, &target) || again != 1 {
        return;
    }
    let rests = w12a_said_again(tail, &targets, noun);
    for lead in with_effects(&value, std::slice::from_ref(first)) {
        if lead.ends_with('.') {
            texts.extend(rests.iter().map(|rest| format!("{lead} {rest}")));
        }
    }
}

/// The targets of the ability when it has exactly one.
fn w12a_targets(value: &Value) -> Option<Vec<Value>> {
    let inner = value.as_object()?.values().next()?;
    let targets = inner.get("targets")?.as_array()?;
    (targets.len() == 1).then(|| targets.clone())
}

/// The noun of a plain target ("creature", "player"); `None` for a target with more to it than one type.
fn w12a_noun(target: &Value) -> Option<&'static str> {
    let what = target.as_object()?;
    if what.len() != 1 {
        return None;
    }
    match what.get("what")? {
        Value::String(kind) if kind == "Player" => Some("player"),
        Value::String(kind) if kind == "Opponent" => Some("opponent"),
        Value::Object(object) => match object.get("Object")?.as_array()?.as_slice() {
            [only] => match only.get("Type")?.as_str()? {
                "Creature" => Some("creature"),
                "Artifact" => Some("artifact"),
                "Land" => Some("land"),
                "Enchantment" => Some("enchantment"),
                "Planeswalker" => Some("planeswalker"),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

/// The sentences of the later steps with the target said again: "it" for an object, "that creature", "that player".
fn w12a_said_again(tail: &[Value], targets: &[Value], noun: &str) -> Vec<String> {
    let named = format!("target {noun}");
    let mut forms = vec![format!("that {noun}")];
    if !matches!(noun, "player" | "opponent") {
        forms.push("it".to_owned());
    }
    let mut out = Vec::new();
    for text in wordings(json!({"Spell": {"effects": tail, "targets": targets}})) {
        if text.matches(&named).count() + text.matches(&capitalized(&named)).count() != 1 {
            continue;
        }
        for form in &forms {
            let said = text
                .replace(&named, form)
                .replace(&capitalized(&named), &capitalized(form));
            out.push(capitalized(&said));
        }
    }
    out
}
