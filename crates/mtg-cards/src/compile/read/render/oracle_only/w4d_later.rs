//! A delayed trigger that closes a spell, an activated or a triggered ability: "Counter target spell. At the beginning
//! of your next first main phase, add X mana ...", "{T}: When target creature other than this creature dies this turn,
//! return that card to the battlefield under your control." The sentence is the trigger and effects of the delayed
//! trigger written as an ability of their own, the object it waits for written as `~` and then named back.

use serde_json::{Value, json};

use super::twin_json::lowered;
use super::w4d_parts::{
    capitalized, clauses, heads, joined, parts, swapped, with_effects, wordings,
};
use crate::ability::Ability;

/// Adds the wordings of an ability whose last effect is a delayed trigger.
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
    let after_lead = !before.is_empty();
    let (reference, names) = referred(&value, delayed, after_lead);
    let later = later_sentences(delayed, reference.as_ref(), &names);
    if later.is_empty() {
        return;
    }
    let leads = if after_lead {
        leads(&value, before, &names)
    } else {
        heads(&value)
    };
    let mut said: Vec<String> = leads
        .iter()
        .flat_map(|lead| {
            later.iter().map(move |sentence| {
                if after_lead {
                    format!("{lead} {sentence}")
                } else {
                    joined(lead, sentence)
                }
            })
        })
        .collect();
    said.sort();
    said.dedup();
    texts.extend(said);
}

/// The wordings of the effects before the delayed trigger; a closing grant of a keyword to the object for good is
/// its own sentence ("That creature gains haste.").
fn leads(value: &Value, before: &[Value], names: &[String]) -> Vec<String> {
    let texts = with_effects(value, before);
    if !texts.is_empty() {
        return texts;
    }
    let Some((grant, first)) = before.split_last() else {
        return Vec::new();
    };
    let Some(keyword) = granted(grant) else {
        return Vec::new();
    };
    let firsts = with_effects(value, first);
    firsts
        .iter()
        .flat_map(|text| {
            names
                .iter()
                .map(|name| format!("{text} {} gains {keyword}.", capitalized(name)))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The keyword a continuous effect grants for good, in lower case words ("first strike").
fn granted(effect: &Value) -> Option<String> {
    let grant = effect.get("Continuous")?;
    (grant.get("duration")? == "Forever").then_some(())?;
    let [modification] = grant.get("mods")?.as_array()?.as_slice() else {
        return None;
    };
    let name = modification.get("AddKeyword")?.as_str()?;
    let mut words = String::new();
    for letter in name.chars() {
        if letter.is_uppercase() && !words.is_empty() {
            words.push(' ');
        }
        words.extend(letter.to_lowercase());
    }
    Some(words)
}

/// The reference the delayed trigger waits for or acts on (a target or a bound object) and the ways to name it: back
/// as "it" or "that creature" after a sentence named it, else as the target itself.
fn referred(value: &Value, delayed: &Value, after_lead: bool) -> (Option<Value>, Vec<String>) {
    let said = delayed.to_string();
    let mut value = value.clone();
    let targets = super::twin_json::inner_mut(&mut value)
        .and_then(|inner| inner.get("targets"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for (at, target) in targets.iter().enumerate() {
        let reference = json!({"Target": at});
        if !said.contains(&reference.to_string()) {
            continue;
        }
        let callings = if after_lead {
            vec!["it".to_owned(), format!("that {}", noun(target))]
        } else {
            described(&targets, &reference)
        };
        return (Some(reference), callings);
    }
    (None, Vec::new())
}

/// The kind a target is named back by: "spell" or its first card type ("creature").
fn noun(target: &Value) -> String {
    let filter = target.pointer("/what/Object").and_then(Value::as_array);
    let filter = filter.map(Vec::as_slice).unwrap_or_default();
    if filter.iter().any(|part| part == "Spell") {
        return "spell".to_owned();
    }
    filter
        .iter()
        .find_map(|part| part.get("Type").and_then(Value::as_str))
        .map_or_else(|| "card".to_owned(), str::to_lowercase)
}

/// The target as the renderers name it ("target creature other than this creature").
fn described(targets: &[Value], reference: &Value) -> Vec<String> {
    wordings(json!({"Spell": {"targets": targets, "effects": [{"Destroy": {"what": reference}}]}}))
        .iter()
        .filter_map(|text| text.strip_prefix("Destroy ")?.strip_suffix('.'))
        .map(str::to_owned)
        .collect()
}

/// The sentences of the delayed trigger, its object named back by each of `names`.
fn later_sentences(delayed: &Value, reference: Option<&Value>, names: &[String]) -> Vec<String> {
    let mut when = delayed.get("when").cloned().unwrap_or_default();
    let mut effects = delayed.get("effects").cloned().unwrap_or_default();
    let this = json!("This");
    if let Some(reference) = reference {
        if delayed.to_string().contains("\"This\"") {
            return Vec::new();
        }
        swapped(&mut when, reference, &this);
        swapped(&mut effects, reference, &this);
    }
    let once = delayed.get("once").and_then(Value::as_bool).unwrap_or(true);
    let this_turn = delayed
        .get("this_turn")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let Some(effects) = effects.as_array() else {
        return Vec::new();
    };
    let step = when.get("StepBegins").is_some();
    let heads = later_heads(&when, once, this_turn, step);
    let bodies = bodies(effects);
    let name_back = |text: &str| -> Vec<String> {
        if reference.is_none() || !text.contains('~') {
            return vec![text.to_owned()];
        }
        names.iter().map(|name| named_back(text, name)).collect()
    };
    let mut sentences = Vec::new();
    for head in &heads {
        for body in &bodies {
            for text in name_back(&format!("{head}{body}.")) {
                sentences.push(capitalized(&text));
            }
            if step {
                let trailing = format!("{body} {}.", lowered(head.trim_end_matches(", ")));
                sentences.extend(name_back(&trailing).iter().map(|text| capitalized(text)));
            }
        }
    }
    sentences
}

/// The text with `~` named back by `name`, and `~'s` by its possessive ("its", "that spell's").
fn named_back(text: &str, name: &str) -> String {
    let possessive = if name == "it" {
        "its".to_owned()
    } else {
        format!("{name}'s")
    };
    text.replace("~'s", &possessive).replace('~', name)
}

/// The heads of the delayed trigger: "At the beginning of the next end step, ", "When ~ dies this turn, ".
fn later_heads(when: &Value, once: bool, this_turn: bool, step: bool) -> Vec<String> {
    let triggered = json!({"Triggered": {"when": when, "effects": [{"Draw": {}}]}});
    let heads = heads(&triggered);
    if step {
        if !once || this_turn {
            return Vec::new();
        }
        return heads.iter().filter_map(|head| next_step(head)).collect();
    }
    heads
        .iter()
        .filter_map(|head| {
            let head = match (once, head.strip_prefix("Whenever ")) {
                (true, Some(rest)) => format!("When {rest}"),
                (false, None) => format!("Whenever {}", head.strip_prefix("When ")?),
                _ => head.clone(),
            };
            let event = head.strip_suffix(", ")?;
            Some(if this_turn {
                format!("{event} this turn, ")
            } else {
                head
            })
        })
        .collect()
}

/// The head of a step said of its next time: "each end step" becomes "the next end step", "your end step" becomes "your
/// next end step"; `None` for the heads of each player's or each of your steps.
fn next_step(head: &str) -> Option<String> {
    if head.contains("each player's") || head.contains("each of your") {
        return None;
    }
    if let Some(rest) = head.strip_prefix("At the beginning of each ") {
        return Some(format!("At the beginning of the next {rest}"));
    }
    let rest = head.strip_prefix("At the beginning of your ")?;
    Some(format!("At the beginning of your next {rest}"))
}

/// The wordings of the effects of the delayed trigger, or of the one effect the renderers do not write alone.
fn bodies(effects: &[Value]) -> Vec<String> {
    let mut texts = clauses(effects);
    if let [effect] = effects {
        texts.extend(lone_effect(effect));
    }
    let others: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("each other creature"))
        .map(|text| text.replace("each other creature", "each creature other than ~"))
        .collect();
    texts.extend(others);
    texts
}

/// The wording of one effect a delayed trigger names and no renderer writes alone: "you get an experience counter",
/// "return that card to the battlefield under your control", "sacrifice ~", "destroy that creature".
pub(super) fn lone_effect(effect: &Value) -> Option<String> {
    if let Some(put) = effect.get("PutCounters") {
        let name = put.pointer("/counter/Other")?.as_str()?;
        let one = put.get("count").is_none_or(|count| count == 1);
        let article = if name.starts_with(['a', 'e', 'i', 'o', 'u']) {
            "an"
        } else {
            "a"
        };
        return (put.get("on")? == "You" && one)
            .then(|| format!("you get {article} {name} counter"));
    }
    if *effect == json!({"Move": {"what": "Triggering", "to": {"Battlefield": {"under": "You"}}}}) {
        return Some("return that card to the battlefield under your control".to_owned());
    }
    if *effect == json!({"Sacrifice": {"filter": ["This"]}}) {
        return Some("sacrifice ~".to_owned());
    }
    (*effect == json!({"Destroy": {"what": "Triggering"}}))
        .then(|| "destroy that creature".to_owned())
}
