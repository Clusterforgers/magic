//! A step repeated for each player or each opponent that closes an ability: "Each player searches their library for a
//! basic land card, puts it onto the battlefield, then shuffles.", "Each opponent who has three or more poison counters
//! loses 3 life.", "Then for each opponent who has less life than you, create a ...", "Each player may draw a card,
//! then each player who drew a card this way gains 1 life." The step is written for you and said of the group.

use serde_json::{Value, json};

use super::twin_json::lowered;
use super::w4d_parts::{
    capitalized, clauses, heads, joined, parts, swapped, third_person, with_effects,
};
use crate::ability::Ability;
use crate::compile::read::amounts::count_word;

/// Adds the wordings of an ability whose last effect repeats its steps for each player of a group.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((value, effects)) = parts(ability) else {
        return;
    };
    let Some((last, before)) = effects.split_last() else {
        return;
    };
    let Some(each) = last.get("RepeatEach") else {
        return;
    };
    let Some(member) = each.get("over").and_then(Value::as_str).and_then(member) else {
        return;
    };
    let (Some(name), Some(steps)) = (
        each.get("as"),
        each.get("effects").and_then(Value::as_array),
    ) else {
        return;
    };
    let bound = json!({"Bound": name});
    let sentences: Vec<String> = sentences(steps, &bound, member)
        .iter()
        .map(|sentence| format!("{sentence}."))
        .collect();
    if before.is_empty() {
        for head in heads(&value) {
            texts.extend(sentences.iter().map(|sentence| joined(&head, sentence)));
        }
        return;
    }
    for lead in with_effects(&value, before) {
        for sentence in &sentences {
            texts.push(format!("{lead} {}", capitalized(sentence)));
            texts.push(format!("{lead}\n{}", capitalized(sentence)));
            if sentence.starts_with("for each") {
                texts.push(format!("{lead} Then {sentence}"));
            }
        }
    }
}

/// The member of the group a step repeats over: "player", "opponent".
fn member(over: &str) -> Option<&'static str> {
    match over {
        "EachPlayer" => Some("player"),
        "Opponents" => Some("opponent"),
        _ => None,
    }
}

/// The steps written for you, the player they repeat for in their place.
fn for_you(steps: &[Value], bound: &Value) -> Vec<String> {
    let mut steps = Value::Array(steps.to_vec());
    swapped(&mut steps, bound, &json!("You"));
    steps
        .as_array()
        .map(|steps| clauses(steps))
        .unwrap_or_default()
}

/// The sentences of the repeated steps, in lower case without the closing period.
fn sentences(steps: &[Value], bound: &Value, member: &str) -> Vec<String> {
    if let [step] = steps
        && let Some(test) = step.get("If")
        && test.get("else").is_none_or(|other| other == &json!([]))
    {
        return filtered(test, bound, member);
    }
    if let [chance, after] = steps
        && let Some(texts) = performed(chance, after, bound, member)
    {
        return texts;
    }
    for_you(steps, bound)
        .iter()
        .filter_map(|text| third_person(text, &format!("each {member}")))
        .collect()
}

/// The steps for the players who meet a condition: "each opponent who has ... loses 3 life" when the step is theirs,
/// "for each opponent who has ..., create ..." when it is yours.
fn filtered(test: &Value, bound: &Value, member: &str) -> Vec<String> {
    let (Some(condition), Some(then)) = (
        test.get("condition"),
        test.get("then").and_then(Value::as_array),
    ) else {
        return Vec::new();
    };
    let Some(has) = condition_of(condition, bound) else {
        return Vec::new();
    };
    let theirs = Value::Array(then.clone())
        .to_string()
        .contains(&bound.to_string());
    let subject = format!("each {member} who {has}");
    if theirs {
        return for_you(then, bound)
            .iter()
            .filter_map(|text| third_person(text, &subject))
            .collect();
    }
    clauses(then)
        .iter()
        .map(|text| format!("for {subject}, {text}"))
        .collect()
}

/// A condition on the player a step repeats for: "has three or more poison counters", "has less life than you".
fn condition_of(condition: &Value, bound: &Value) -> Option<String> {
    let compare = condition.get("Compare")?;
    let (left, op, right) = (
        compare.get("left")?,
        compare.get("op")?.as_str()?,
        compare.get("right")?,
    );
    if let Some(counters) = left.get("Counters") {
        (counters.get("on")? == bound && op == "Ge").then_some(())?;
        let kind = counters.get("counter")?;
        let kind = kind.as_str().or_else(|| kind.get("Other")?.as_str())?;
        let count = count_word(u32::try_from(right.as_u64()?).ok()?)?;
        return Some(format!("has {count} or more {kind} counters"));
    }
    (left.get("Life")? == bound && right == &json!({"Life": "You"})).then_some(())?;
    match op {
        "Lt" => Some("has less life than you".to_owned()),
        "Gt" => Some("has more life than you".to_owned()),
        _ => None,
    }
}

/// A step each player may take and what each player who took it gets: "each player may draw a card, then each player
/// who drew a card this way gains 1 life".
fn performed(chance: &Value, after: &Value, bound: &Value, member: &str) -> Option<Vec<String>> {
    let chance = chance.get("May")?;
    (chance.get("who")? == bound).then_some(())?;
    let after = after.get("If")?;
    (after.get("condition")? == "Performed").then_some(())?;
    let taken = for_you(chance.get("effects")?.as_array()?, bound);
    let then = after.get("then")?.as_array()?;
    let mut texts = Vec::new();
    for step in &taken {
        let step = lowered(step.strip_prefix("you ").unwrap_or(step));
        let Some(done) = past(&step) else {
            continue;
        };
        let subject = format!("each {member} who {done} this way");
        for gained in for_you(then, bound) {
            if let Some(gained) = third_person(&gained, &subject) {
                texts.push(format!("each {member} may {step}, then {gained}"));
            }
        }
    }
    Some(texts)
}

/// The step in the past tense: "draw a card" becomes "drew a card".
fn past(step: &str) -> Option<String> {
    let (verb, rest) = step.split_once(' ')?;
    let done = match verb {
        "draw" => "drew",
        "discard" => "discarded",
        "sacrifice" => "sacrificed",
        "mill" => "milled",
        _ => return None,
    };
    Some(format!("{done} {rest}"))
}
