//! Cards put onto the battlefield attached to a player: "you may search your library for a Curse card, put it onto the
//! battlefield attached to target player, then shuffle.", "return this card to the battlefield transformed under your
//! control attached to target opponent", "return it to the battlefield attached to you". The wording is the twin that puts
//! the card onto the plain battlefield, without the target, with the player's words after the destination. Auras put
//! onto the battlefield that may enchant only some permanents: "Return each Aura card from your graveyard to the
//! battlefield. Only creatures can be enchanted this way.", the plain move and a sentence of what may be enchanted.

use super::super::super::selectors::{Form, phrase_text};
use super::super::variants;
use super::twin_json::{inner_mut, twin};
use crate::ability::Ability;
use crate::refs::Selector;
use serde_json::{Value, json};

/// Adds the attached-to-player and enchanting wordings.
pub(super) fn r5515_adds(ability: &Ability, texts: &mut Vec<String>) {
    r5515_add(ability, texts);
    r5515_add_enchanting(ability, texts);
}

/// Adds the wordings of an ability that puts a card onto the battlefield attached to a player: a target player or
/// opponent the ability names only there, or "you". The twin puts it onto the plain battlefield (transformed and under
/// whose control kept) and the player's words follow the destination.
fn r5515_add(ability: &Ability, texts: &mut Vec<String>) {
    let mut found = None;
    let Some(plain) = twin(ability, |value| {
        let body = inner_mut(value)?;
        let mut hosts = Vec::new();
        r5515_plain_hosts(body, &mut hosts);
        let [(to, transformed, under)] = hosts.as_slice() else {
            return None;
        };
        let words = if to == "You" {
            "you".to_owned()
        } else {
            let index = to.get("Target")?.as_u64()?;
            let targets = body.get_mut("targets")?.as_array_mut()?;
            if index != 0 || targets.len() != 1 {
                return None;
            }
            let words = match targets.first()?.get("what")?.as_str()? {
                "Player" => "target player",
                "Opponent" => "target opponent",
                _ => return None,
            };
            body.as_object_mut()?.remove("targets");
            if r5515_names_target(body) {
                return None;
            }
            words.to_owned()
        };
        let mut lead = "the battlefield".to_owned();
        if *transformed {
            lead += " transformed";
        }
        match under {
            None => {}
            Some(who) if who == "You" => lead += " under your control",
            Some(_) => return None,
        }
        found = Some((lead, words));
        Some(())
    }) else {
        return;
    };
    let Some((lead, words)) = found else {
        return;
    };
    for text in variants(&plain) {
        if text.matches(lead.as_str()).count() == 1 {
            texts.push(text.replacen(&lead, &format!("{lead} attached to {words}"), 1));
        }
    }
}

/// Puts the plain battlefield in place of every move onto the battlefield attached to a player named by a target or
/// "you", and lists each one's player, whether it enters transformed and under whose control.
fn r5515_plain_hosts(value: &mut Value, hosts: &mut Vec<(Value, bool, Option<Value>)>) {
    match value {
        Value::Object(map) => {
            let attached = map
                .get("to")
                .and_then(|to| to.get("BattlefieldAttached"))
                .cloned();
            if let Some(attached) = attached {
                let to = attached.get("to").cloned().unwrap_or(Value::Null);
                if to == "You" || to.get("Target").is_some() {
                    let transformed = attached.get("transformed") == Some(&Value::Bool(true));
                    let under = attached.get("under").cloned();
                    let mut plain = serde_json::Map::new();
                    if transformed {
                        plain.insert("transformed".to_owned(), Value::Bool(true));
                    }
                    if let Some(under) = &under {
                        plain.insert("under".to_owned(), under.clone());
                    }
                    map.insert("to".to_owned(), json!({ "Battlefield": plain }));
                    hosts.push((to, transformed, under));
                    return;
                }
            }
            for inner in map.values_mut() {
                r5515_plain_hosts(inner, hosts);
            }
        }
        Value::Array(list) => {
            for inner in list {
                r5515_plain_hosts(inner, hosts);
            }
        }
        _ => {}
    }
}

/// Whether a reference to a target is left.
fn r5515_names_target(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.contains_key("Target") || map.values().any(r5515_names_target),
        Value::Array(list) => list.iter().any(r5515_names_target),
        _ => false,
    }
}

/// Adds the wordings of a spell whose one step returns Auras to the battlefield enchanting only what a selector matches.
fn r5515_add_enchanting(ability: &Ability, texts: &mut Vec<String>) {
    let mut only = None;
    let Some(plain) = twin(ability, |value| {
        let body = inner_mut(value)?;
        let [step] = body.get_mut("effects")?.as_array_mut()?.as_mut_slice() else {
            return None;
        };
        let to = step.get_mut("Move")?.get_mut("to")?;
        only = Some(to.get("BattlefieldEnchanting")?.get("only")?.clone());
        *to = json!({"Battlefield": {}});
        Some(())
    }) else {
        return;
    };
    let Some(only) = only.and_then(|only| serde_json::from_value::<Selector>(only).ok()) else {
        return;
    };
    let Some(what) = phrase_text(&only, Form::All) else {
        return;
    };
    let rule = format!(" Only {what} can be enchanted this way.");
    for text in variants(&plain) {
        if text.ends_with(" to the battlefield.") {
            texts.push(format!("{text}{rule}"));
        }
    }
}
