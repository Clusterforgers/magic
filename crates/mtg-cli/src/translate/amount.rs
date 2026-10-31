//! Amounts: Forge's numbers, `X` and the `Count$` expressions behind a named part, in the DSL's `Amount`.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::valid::{defined, selector_in};

/// The DSL name of a counter kind (`P1P1` is `+1/+1`).
pub fn counter_kind(word: &str) -> Option<Value> {
    let word = word.trim();
    if word.is_empty() || !word.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
        return None;
    }
    Some(match word {
        "P1P1" => json!("+1/+1"),
        "M1M1" => json!("-1/-1"),
        "LOYALTY" => json!("loyalty"),
        "POISON" => json!("poison"),
        "CHARGE" => json!("charge"),
        other => json!({"Other": other.to_lowercase().replace('_', " ")}),
    })
}

/// The amount a Forge value stands for: a number, `X`, or the name of a part that holds an expression.
pub fn amount(ctx: &Ctx, text: &str) -> Tr<Value> {
    let text = text.trim();
    let text = if text.starts_with("+") && text.len() > 1 { &text[1..] } else { text };
    if let Ok(number) = text.parse::<i32>() {
        return Ok(json!(number));
    }
    if let Some(inner) = text.strip_prefix('-')
        && let Some(value) = negate(ctx, inner)
    {
        return Ok(value);
    }
    match ctx.svar(text) {
        Some(expression) => expression_amount(ctx, expression),
        None => unmapped(format!("amount:{text}")),
    }
}

/// `-X` and `-N`: the amount times minus one.
fn negate(ctx: &Ctx, inner: &str) -> Option<Value> {
    let value = amount(ctx, inner).ok()?;
    match value.as_i64() {
        Some(number) => Some(json!(-number)),
        None => Some(json!({"Times": {"by": -1, "of": value}})),
    }
}

/// The amount of the value of a named part.
fn expression_amount(ctx: &Ctx, expression: &str) -> Tr<Value> {
    let expression = expression.trim();
    if let Ok(number) = expression.parse::<i32>() {
        return Ok(json!(number));
    }
    if let Some(rest) = expression.strip_prefix("Number$") {
        return rest
            .trim()
            .parse::<i32>()
            .map(|number| json!(number))
            .or_else(|_| unmapped("amount:Number$"));
    }
    if let Some(rest) = expression.strip_prefix("Count$") {
        return count_amount(ctx, rest);
    }
    if let Some(rest) = expression.strip_prefix("Targeted$") {
        return targeted_amount(ctx, rest);
    }
    unmapped(format!("amount:{}", head(expression)))
}

/// The part of an expression before its first `$`, space or dot, for naming a gap.
fn head(expression: &str) -> String {
    expression
        .split(['$', ' ', '.', '/'])
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// A stat of the latest target.
fn targeted_amount(ctx: &Ctx, what: &str) -> Tr<Value> {
    let stat = match what {
        "CardPower" => "Power",
        "CardToughness" => "Toughness",
        "CardManaCost" => "ManaValue",
        other => return unmapped(format!("amount:Targeted${other}")),
    };
    Ok(json!({"Stat": {"of": ctx.last_target()?, "stat": stat}}))
}

/// The amount of a `Count$` expression: its base and the `/Plus.N`, `/Minus.N`, `/Twice` or `/Times.N` steps after it.
fn count_amount(ctx: &Ctx, expression: &str) -> Tr<Value> {
    let mut parts = expression.split('/');
    let first = parts.next().unwrap_or_default();
    let mut value = count_base(ctx, first)?;
    for step in parts {
        value = match step.split_once('.') {
            Some(("Plus", n)) => sum(value, parse_number(n)?),
            Some(("Minus", n)) => sum(value, -parse_number(n)?),
            Some(("Times", n)) => times(value, parse_number(n)?),
            None if step == "Twice" => times(value, 2),
            _ => return unmapped(format!("amount:Count/{}", head(step))),
        };
    }
    Ok(value)
}

fn parse_number(text: &str) -> Tr<i32> {
    text.parse().or_else(|_| unmapped("amount:step-number"))
}

fn sum(value: Value, n: i32) -> Value {
    if let Some(number) = value.as_i64().and_then(|v| i32::try_from(v).ok()) {
        return json!(number + n);
    }
    json!({"Sum": [value, n]})
}

fn times(value: Value, n: i32) -> Value {
    json!({"Times": {"by": n, "of": value}})
}

/// The base of a `Count$` expression.
fn count_base(ctx: &Ctx, base: &str) -> Tr<Value> {
    let (word, rest) = base.split_once(' ').unwrap_or((base, ""));
    match word {
        "xPaid" => Ok(json!("X")),
        "Valid" => Ok(json!({"Count": selector_in(ctx, rest, None)?})),
        "ValidGraveyard" => Ok(json!({"Count": selector_in(ctx, rest, Some("Graveyard"))?})),
        "ValidHand" => Ok(json!({"Count": selector_in(ctx, rest, Some("Hand"))?})),
        "ValidExile" => Ok(json!({"Count": selector_in(ctx, rest, Some("Exile"))?})),
        "CardPower" => stat_of_this("Power"),
        "CardToughness" => stat_of_this("Toughness"),
        "CardManaCost" => stat_of_this("ManaValue"),
        "YourLifeTotal" => Ok(json!({"Life": "You"})),
        "OppLifeTotal" | "TargetedPlayerLife" => unmapped(format!("amount:Count${word}")),
        "TimesKicked" => Ok(json!("Kicks")),
        other => counters_or_gap(ctx, other),
    }
}

fn stat_of_this(stat: &str) -> Tr<Value> {
    Ok(json!({"Stat": {"of": "This", "stat": stat}}))
}

/// `CardCounters.P1P1` (counters on this card), or the gap that names the base.
fn counters_or_gap(ctx: &Ctx, word: &str) -> Tr<Value> {
    if let Some(kind) = word
        .strip_prefix("CardCounters.")
        .and_then(counter_kind)
    {
        let on = defined(ctx, "Self")?;
        return Ok(json!({"Counters": {"on": on, "counter": kind}}));
    }
    unmapped(format!("amount:Count${}", head(word)))
}
