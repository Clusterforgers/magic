//! Cards put onto the battlefield attached to a permanent: "Return target creature card from your graveyard to the
//! battlefield, then return up to two target Aura and/or Equipment cards from your graveyard to the battlefield attached
//! to that creature.", "When ~ enters, you may put an Aura or Equipment card from your hand or graveyard onto the
//! battlefield attached to ~." The wording is the plain move onto the battlefield and the host.

use super::super::variants;
use super::twin_json::twin;
use crate::ability::Ability;
use serde_json::{Value, json};

/// Adds the wordings of an ability whose last step puts cards onto the battlefield attached to a permanent.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let mut hosts = Vec::new();
    let plain = twin(ability, |value| {
        hosts = plain_destinations(value);
        (hosts.len() == 1).then_some(())
    });
    let Some(plain) = plain else {
        return;
    };
    let Some(host) = hosts
        .first()
        .and_then(|to| host_words(&serde_json::to_value(ability).ok()?, to))
    else {
        return;
    };
    for text in variants(&plain) {
        let Some(head) = text.strip_suffix(" the battlefield.") else {
            continue;
        };
        let said = format!("{head} the battlefield attached to {host}.");
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
            if let Some(Value::Object(attached)) = map.get("to")
                && let Some(to) = attached
                    .get("BattlefieldAttached")
                    .and_then(|inner| inner.get("to"))
            {
                found.push(to.clone());
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
