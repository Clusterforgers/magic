//! Mana and cost wordings built from stand-ins: mana added a number of times ("Add an amount of {B} equal to the
//! sacrificed creature's mana value", the amount worded as a gain of life), mana of a type a group of permanents could
//! produce ("one mana of any type that a Gate you control could produce") and a cost change for the spells or abilities
//! that target a group ("Abilities your opponents activate that target a Merfolk you control cost {2} more to activate").

use serde_json::{Value, json};

use super::super::article;
use super::twin_json::inner_mut;
use super::w4d_parts::{parts, wordings};
use super::w5b_parts::w5b_noun;
use crate::ability::Ability;

/// Adds the wordings of mana added a number of times, mana a group could produce and a cost change for targeting.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if let Some((value, effects)) = parts(ability)
        && let [step] = effects.as_slice()
        && let Some(mana) = step.get("AddMana")
    {
        texts.extend(times(&value, mana));
        texts.extend(produced_by(&value, mana));
    }
    texts.extend(targeting(ability));
}

/// The ability with `step` as its one effect, worded.
fn with_step(value: &Value, step: Value) -> Vec<String> {
    let mut value = value.clone();
    let Some(inner) = inner_mut(&mut value).and_then(Value::as_object_mut) else {
        return Vec::new();
    };
    inner.insert("effects".to_owned(), json!([step]));
    wordings(value)
}

/// "Add an amount of {B} equal to Y", Y the words of a gain of life of the same amount.
fn times(value: &Value, mana: &Value) -> Vec<String> {
    let Some(amount) = mana.get("times") else {
        return Vec::new();
    };
    let mut once = mana.clone();
    if let Some(map) = once.as_object_mut() {
        map.remove("times");
    }
    let gains = with_step(value, json!({"GainLife": {"amount": amount}}));
    let mut found = Vec::new();
    for added in with_step(value, json!({"AddMana": once})) {
        let Some((head, symbols)) = added.rsplit_once("Add ") else {
            continue;
        };
        let Some(symbols) = symbols.strip_suffix('.') else {
            continue;
        };
        for gain in &gains {
            if let Some(equal) = gain
                .strip_prefix(head)
                .and_then(|rest| rest.strip_prefix("You gain life equal to "))
            {
                found.push(format!("{head}Add an amount of {symbols} equal to {equal}"));
            }
        }
    }
    found
}

/// "one mana of any type that a Gate you control could produce": the wording for lands with the group's noun.
fn produced_by(value: &Value, mana: &Value) -> Vec<String> {
    let Some(among) = mana
        .get("mana")
        .and_then(|produce| produce.get("ProducedBy"))
        .and_then(|produced| produced.get("among"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let Some((owner, kinds)) = among.split_last() else {
        return Vec::new();
    };
    let who = match owner.get("ControlledBy").and_then(Value::as_str) {
        Some("You") => "you control",
        Some("Opponents") => "an opponent controls",
        _ => return Vec::new(),
    };
    if kinds.is_empty() || kinds == [json!({"Type": "Land"})] {
        return Vec::new();
    }
    let mut lands = mana.clone();
    if let Some(produced) = lands.pointer_mut("/mana/ProducedBy") {
        produced["among"] = json!([{"Type": "Land"}, owner]);
    }
    let land = format!("a land {who} could produce");
    let mut found = Vec::new();
    for noun in w5b_noun(&Value::Array(among.clone())) {
        for text in with_step(value, json!({"AddMana": lands})) {
            if text.contains(&land) {
                found
                    .push(text.replace(&land, &format!("{} {noun} could produce", article(&noun))));
            }
        }
    }
    found
}

/// "Spells your opponents cast that target a Merfolk you control cost {2} more to cast." and the same of abilities.
fn targeting(ability: &Ability) -> Vec<String> {
    let Ok(value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    let Some(modifier) = value.get("CostModifier") else {
        return Vec::new();
    };
    if modifier.get("per").is_some() || modifier.get("condition").is_some() {
        return Vec::new();
    }
    let Some([kind, owner, aim]) = modifier
        .get("applies_to")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .and_then(|list| <&[Value; 3]>::try_from(list).ok())
    else {
        return Vec::new();
    };
    let (things, verb) = if *kind == json!("Spell") {
        ("Spells", "cast")
    } else if *kind == json!({"Ability": "Activated"}) {
        ("Abilities", "activate")
    } else {
        return Vec::new();
    };
    let who = match owner.get("ControlledBy").and_then(Value::as_str) {
        Some("You") => "you",
        Some("Opponents") => "your opponents",
        _ => return Vec::new(),
    };
    let (Some(selector), Some(mana)) = (
        aim.get("Targeting"),
        modifier.get("mana").and_then(Value::as_str),
    ) else {
        return Vec::new();
    };
    let more = match modifier.get("change").and_then(Value::as_str) {
        Some("Increase") => "more",
        Some("Reduce") => "less",
        _ => return Vec::new(),
    };
    let aims: Vec<String> = if *selector == json!(["This"]) {
        vec!["~".to_owned()]
    } else {
        w5b_noun(selector)
            .iter()
            .map(|noun| format!("{} {noun}", article(noun)))
            .collect()
    };
    aims.iter()
        .map(|aim| format!("{things} {who} {verb} that target {aim} cost {mana} {more} to {verb}."))
        .collect()
}
