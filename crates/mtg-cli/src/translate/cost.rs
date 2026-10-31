//! Costs: Forge's `Cost$` words (`2 R T Sac<1/Creature>`) as the DSL's cost items, and loyalty costs.

use serde_json::{Value, json};

use super::amount::{amount, counter_kind};
use super::ctx::{Ctx, Tr, unmapped};
use super::valid::selector_any;

/// The parts of a cost: words split at spaces outside angle brackets.
pub fn parts(cost: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (at, c) in cost.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            c if c.is_whitespace() && depth == 0 => {
                if start < at {
                    parts.push(&cost[start..at]);
                }
                start = at + c.len_utf8();
            }
            _ => {}
        }
    }
    if start < cost.len() {
        parts.push(&cost[start..]);
    }
    parts
}

/// Whether a cost word is a mana symbol (`2`, `R`, `W/U`, `X`).
pub fn is_mana(word: &str) -> bool {
    !word.is_empty()
        && (word.chars().all(|c| c.is_ascii_digit())
            || (word.len() <= 3
                && word
                    .split('/')
                    .all(|side| side.len() == 1 && "WUBRGCXSP".contains(side) || side == "2")))
}

/// The loyalty cost a word stands for (`AddCounter<2/LOYALTY>`).
fn loyalty_word(word: &str) -> Option<Value> {
    let (head, rest) = word.split_once('<')?;
    let inner = rest.strip_suffix('>')?;
    let (number, kind) = inner.split_once('/')?;
    if !kind.eq_ignore_ascii_case("loyalty") {
        return None;
    }
    match (head, number) {
        ("AddCounter", "0") | ("SubCounter", "0") => Some(json!("Zero")),
        ("AddCounter", n) => n.parse::<u32>().ok().map(|n| json!({"Plus": n})),
        ("SubCounter", "X") => Some(json!("MinusX")),
        ("SubCounter", n) => n.parse::<u32>().ok().map(|n| json!({"Minus": n})),
        _ => None,
    }
}

/// The loyalty cost of an ability whose whole cost is one loyalty word.
pub fn loyalty(cost: &str) -> Option<Value> {
    match parts(cost).as_slice() {
        [word] => loyalty_word(word),
        _ => None,
    }
}

/// The cost items a cost stands for, as the DSL's `Cost`: a string for mana alone.
pub fn cost(ctx: &Ctx, text: &str) -> Tr<Value> {
    let mut mana = String::new();
    let mut items = Vec::new();
    for word in parts(text) {
        if is_mana(word) {
            mana.push_str(&format!("{{{word}}}"));
        } else {
            items.push(item(ctx, word)?);
        }
    }
    if items.is_empty() {
        return Ok(json!(mana));
    }
    if !mana.is_empty() {
        items.insert(0, json!({ "Mana": mana }));
    }
    Ok(Value::Array(items))
}

/// One non-mana cost word.
fn item(ctx: &Ctx, word: &str) -> Tr<Value> {
    match word {
        "T" => return Ok(json!("Tap")),
        "Q" => return Ok(json!("Untap")),
        _ => {}
    }
    let Some((head, rest)) = word.split_once('<') else {
        return unmapped(format!("cost:{word}"));
    };
    let Some(inner) = rest.strip_suffix('>') else {
        return unmapped(format!("cost:{head}"));
    };
    let mut fields = inner.split('/');
    let number = fields.next().unwrap_or_default();
    let what = fields.next().unwrap_or_default();
    let on_self = matches!(what, "CARDNAME" | "NICKNAME" | "Card.Self");
    match head {
        "Sac" if on_self => Ok(json!("SacrificeSelf")),
        "Sac" => Ok(json!({"Sacrifice": {"filter": selector_any(ctx, what)?, "count": amount(ctx, number)?}})),
        "PayLife" => Ok(json!({"PayLife": amount(ctx, number)?})),
        "PayEnergy" => Ok(json!({"PayEnergy": amount(ctx, number)?})),
        "Mill" => Ok(json!({"Mill": amount(ctx, number)?})),
        "Discard" if on_self => Ok(json!("DiscardSelf")),
        "Discard" => discard(ctx, number, what),
        "Exile" | "ExileFromGrave" if on_self => Ok(json!("ExileSelf")),
        "ExileFromGrave" => zone_cost(ctx, "ExileFromGraveyard", number, what),
        "ExileFromHand" => zone_cost(ctx, "ExileFromHand", number, what),
        "Return" if on_self => unmapped("cost:Return-self"),
        "Return" => zone_cost(ctx, "ReturnToHand", number, what),
        "Reveal" => zone_cost(ctx, "RevealFromHand", number, what),
        "tapXType" => zone_cost(ctx, "TapPermanents", number, what),
        "SubCounter" => remove_counters(ctx, number, what),
        _ => unmapped(format!("cost:{head}")),
    }
}

fn discard(ctx: &Ctx, number: &str, what: &str) -> Tr<Value> {
    let count = amount(ctx, number)?;
    if matches!(what, "Card" | "") {
        return Ok(json!({"Discard": {"count": count}}));
    }
    Ok(json!({"Discard": {"count": count, "filter": selector_any(ctx, what)?}}))
}

fn zone_cost(ctx: &Ctx, variant: &str, number: &str, what: &str) -> Tr<Value> {
    Ok(json!({ variant: {"filter": selector_any(ctx, what)?, "count": amount(ctx, number)?} }))
}

fn remove_counters(ctx: &Ctx, number: &str, what: &str) -> Tr<Value> {
    let Some(kind) = counter_kind(what) else {
        return unmapped("cost:SubCounter-kind");
    };
    Ok(json!({"RemoveCounters": {"counter": kind, "count": amount(ctx, number)?}}))
}
