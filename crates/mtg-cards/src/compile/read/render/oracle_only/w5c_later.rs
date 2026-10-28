//! A delayed trigger at a step said after its effects: "destroy that creature at end of combat", "sacrifice it at the
//! beginning of the next end step". The step reads off the head of a trigger at that step.

use serde_json::{Value, json};

use super::super::super::selectors::{Form, phrase_text};
use super::super::super::triggers::article;
use super::w4d_later::lone_effect;
use super::w4d_parts::{capitalized, clauses, heads, parts, with_effects};
use crate::ability::Ability;
use crate::refs::Selector;

/// Adds the wordings of an ability whose last effect is a delayed trigger at the next time of a step.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((value, effects)) = parts(ability) else {
        return;
    };
    let Some((last, before)) = effects.split_last() else {
        return;
    };
    let Some(delayed) = last.get("DelayedTrigger") else {
        return;
    };
    let once = delayed.get("once").is_none_or(|once| *once == json!(true));
    let this_turn = delayed
        .get("this_turn")
        .is_some_and(|turn| *turn == json!(true));
    let (Some(when), Some(Value::Array(steps))) = (delayed.get("when"), delayed.get("effects"))
    else {
        return;
    };
    if !once || this_turn || when.get("StepBegins").is_none() {
        return;
    }
    let phrases = w5c_step_phrases(when);
    let mut bodies = clauses(steps);
    if let [step] = steps.as_slice() {
        bodies.extend(lone_effect(step));
    }
    let leads = if before.is_empty() {
        w5c_heads(&value)
    } else {
        with_effects(&value, before)
    };
    for lead in &leads {
        for body in &bodies {
            for phrase in &phrases {
                texts.push(w5c_joined(lead, body, phrase, !before.is_empty()));
            }
        }
    }
}

/// The lead and the step's sentence: a sentence of its own after earlier effects, else the effect after the head.
fn w5c_joined(lead: &str, body: &str, phrase: &str, after_lead: bool) -> String {
    if after_lead {
        format!("{lead} {} {phrase}.", capitalized(body))
    } else if lead.ends_with(", ") {
        format!("{lead}{body} {phrase}.")
    } else {
        format!("{lead}{} {phrase}.", capitalized(body))
    }
}

/// The heads of the ability; a trigger on damage to a kind of permanent reads as the one on damage to a player with
/// that kind in place of "a player" ("Whenever ~ deals combat damage to a creature, ").
fn w5c_heads(value: &Value) -> Vec<String> {
    let found = heads(value);
    if !found.is_empty() {
        return found;
    }
    let Some(Value::Array(kind)) = value.pointer("/Triggered/when/DealsDamage/to/Each") else {
        return found;
    };
    let Ok(selector) = serde_json::from_value::<Selector>(Value::Array(kind.clone())) else {
        return found;
    };
    let Some(phrase) = phrase_text(&selector, Form::Subject) else {
        return found;
    };
    let mut player = value.clone();
    if let Some(to) = player.pointer_mut("/Triggered/when/DealsDamage/to") {
        *to = json!("EachPlayer");
    }
    let damaged = format!(" to {} {phrase}, ", article(&phrase));
    heads(&player)
        .iter()
        .filter_map(|head| {
            let lead = head.strip_suffix(" to a player, ")?;
            Some(format!("{lead}{damaged}"))
        })
        .collect()
}

/// The step said after an effect, of a delayed trigger at the next time of any player's step.
fn w5c_step_phrases(when: &Value) -> Vec<String> {
    let Some(begins) = when.get("StepBegins") else {
        return Vec::new();
    };
    if begins
        .get("whose")
        .is_some_and(|whose| *whose != json!("EachPlayer"))
    {
        return Vec::new();
    }
    let phrase = match begins.get("step").and_then(Value::as_str) {
        Some("End") => "at the beginning of the next end step",
        Some("EndCombat") => "at end of combat",
        _ => return Vec::new(),
    };
    vec![phrase.to_owned()]
}
