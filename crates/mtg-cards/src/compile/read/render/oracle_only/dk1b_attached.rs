//! Cards put onto the battlefield attached to a permanent: "Return target creature card from your graveyard to the
//! battlefield, then return up to two target Aura and/or Equipment cards from your graveyard to the battlefield attached
//! to that creature.", "When ~ enters, you may put an Aura or Equipment card from your hand or graveyard onto the
//! battlefield attached to ~." The wording is the plain move onto the battlefield, the zones the card may come from and
//! the permanent it is attached to.

use super::super::variants;
use super::twin_json::twin;
use crate::ability::Ability;
use serde_json::{Value, json};

/// Adds the wordings of an ability whose last step puts cards onto the battlefield attached to a permanent, and of one
/// that puts a card of the hand or the graveyard onto the battlefield.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let mut hosts = Vec::new();
    let mut zones = 0;
    let plain = twin(ability, |value| {
        hosts = plain_destinations(value);
        zones = hand_for_hand_or_graveyard(value);
        (hosts.len() + zones > 0 && hosts.len() <= 1).then_some(())
    });
    let Some(plain) = plain else {
        return;
    };
    let host = match hosts.first() {
        None => None,
        Some(to) => match serde_json::to_value(ability)
            .ok()
            .and_then(|value| host_words(&value, to))
        {
            Some(words) => Some(words),
            None => return,
        },
    };
    for text in variants(&plain) {
        let text = if zones > 0 {
            text.replacen(" from your hand", " from your hand or graveyard", 1)
        } else {
            text
        };
        let said = match &host {
            None => text,
            Some(host) => {
                let Some(head) = text.strip_suffix(" the battlefield.") else {
                    continue;
                };
                format!("{head} the battlefield attached to {host}.")
            }
        };
        texts.extend(and_or(&said));
        texts.push(said);
    }
}

/// Puts the plain destination in place of every move onto the battlefield attached to a permanent, and lists the
/// permanents it was attached to.
fn plain_destinations(value: &mut Value) -> Vec<Value> {
    let mut found = Vec::new();
    match value {
        Value::Object(map) => {
            if let Some(to) = map
                .get("to")
                .and_then(|attached| attached.get("BattlefieldAttached"))
                .and_then(|inner| inner.get("to"))
                .cloned()
            {
                found.push(to);
                map.insert("to".to_owned(), json!({"Battlefield": {}}));
            } else {
                for inner in map.values_mut() {
                    found.extend(plain_destinations(inner));
                }
            }
        }
        Value::Array(list) => {
            for inner in list {
                found.extend(plain_destinations(inner));
            }
        }
        _ => {}
    }
    found
}

/// Puts the hand in place of "the hand or the graveyard" in every selector of cards, first, and counts them.
fn hand_for_hand_or_graveyard(value: &mut Value) -> usize {
    let either = json!({"AnyOf": [{"In": "Hand"}, {"In": "Graveyard"}]});
    let mut count = 0;
    match value {
        Value::Object(map) => {
            for inner in map.values_mut() {
                count += hand_for_hand_or_graveyard(inner);
            }
        }
        Value::Array(list) => {
            if let Some(at) = list.iter().position(|pred| *pred == either) {
                list.remove(at);
                list.insert(0, json!({"In": "Hand"}));
                count += 1;
            }
            for inner in list {
                count += hand_for_hand_or_graveyard(inner);
            }
        }
        _ => {}
    }
    count
}

/// How the text names the permanent: "~" for the card itself, "that creature" for a target the ability chose.
fn host_words(ability: &Value, to: &Value) -> Option<String> {
    if to == "This" {
        return Some("~".to_owned());
    }
    let index = usize::try_from(to.get("Target")?.as_u64()?).ok()?;
    let body = ["Spell", "Triggered", "Activated"]
        .into_iter()
        .find_map(|key| ability.get(key))?;
    let wanted = body
        .get("targets")?
        .get(index)?
        .get("what")?
        .get("Object")?;
    let noun = wanted
        .as_array()?
        .iter()
        .find_map(|pred| pred.get("Type")?.as_str())?;
    Some(format!("that {}", noun.to_lowercase()))
}

/// The wording with "Aura and/or Equipment" for "Aura or Equipment", which Oracle uses for several cards.
fn and_or(text: &str) -> Option<String> {
    text.contains(" Aura or Equipment ")
        .then(|| text.replace(" Aura or Equipment ", " Aura and/or Equipment "))
}
