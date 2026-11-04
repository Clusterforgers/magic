//! Amounts: Forge's numbers, `X` and the `Count$` expressions behind a named part, in the DSL's `Amount`.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::valid::{defined, selector_in};

/// The DSL name of a counter kind (`P1P1` is `+1/+1`).
pub fn counter_kind(word: &str) -> Option<Value> {
    let word = word.trim();
    if word.is_empty()
        || !word
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    {
        return None;
    }
    // "All" is not a kind: the effects that take every counter off say so themselves.
    if word == "ALL" {
        return None;
    }
    Some(match word {
        "P1P1" => json!("+1/+1"),
        "M1M1" => json!("-1/-1"),
        "LOYALTY" => json!("loyalty"),
        "POISON" => json!("poison"),
        "CHARGE" => json!("charge"),
        other if is_power_toughness(other) => json!({"Other": power_toughness_name(other)}),
        other => json!({"Other": other.to_lowercase().replace('_', " ")}),
    })
}

/// The name of a power/toughness counter as the engine reads it in layer 7c: `P2P2` is `+2/+2`, `M0M1` is `-0/-1`.
fn power_toughness_name(word: &str) -> String {
    let mut name = String::new();
    for c in word.chars() {
        match c {
            'P' if name.is_empty() => name.push('+'),
            'M' if name.is_empty() => name.push('-'),
            'P' => name.push_str("/+"),
            'M' => name.push_str("/-"),
            digit => name.push(digit),
        }
    }
    name
}

/// Whether the word is a power/toughness counter (`P2P2`, `M0M1`).
fn is_power_toughness(word: &str) -> bool {
    let mut rest = word;
    for _ in 0..2 {
        let Some(after) = rest.strip_prefix(['P', 'M']) else {
            return false;
        };
        let trimmed = after.trim_start_matches(|c: char| c.is_ascii_digit());
        if trimmed.len() == after.len() {
            return false;
        }
        rest = trimmed;
    }
    rest.is_empty()
}

/// The amount a Forge value stands for: a number, `X`, or the name of a part that holds an expression.
pub fn amount(ctx: &Ctx, text: &str) -> Tr<Value> {
    let text = text.trim();
    let text = if text.starts_with('+') && text.len() > 1 {
        &text[1..]
    } else {
        text
    };
    if let Ok(number) = text.parse::<i32>() {
        return Ok(json!(number));
    }
    if let Some(inner) = text.strip_prefix('-')
        && inner
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
    {
        return negate(ctx, inner);
    }
    match ctx.svar(text) {
        Some(expression) => expression_amount(ctx, expression),
        None => unmapped(format!("amount:{text}")),
    }
}

/// `-X`: the amount times minus one; a part that cannot be read is the gap, not the negation.
fn negate(ctx: &Ctx, inner: &str) -> Tr<Value> {
    let value = amount(ctx, inner)?;
    Ok(match value.as_i64() {
        Some(number) => json!(-number),
        None => json!({"Times": {"by": -1, "of": value}}),
    })
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
    if let Some(rest) = expression.strip_prefix("SVar$") {
        let (name, steps) = rest.split_once('/').unwrap_or((rest, ""));
        return apply_steps(ctx, amount(ctx, name)?, steps);
    }
    let (base, steps) = expression.split_once('/').unwrap_or((expression, ""));
    if let Some(amount) = event_amount(ctx, base)? {
        return apply_steps(ctx, amount, steps);
    }
    if let Some(rest) = expression.strip_prefix("Targeted$") {
        return targeted_amount(rest, &ctx.last_target()?);
    }
    if let Some(rest) = expression.strip_prefix("ParentTargeted$") {
        return targeted_amount(rest, &ctx.parent()?);
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

/// The amounts that name an object of the event or of the cost (`TriggerCount$DamageAmount`, `TriggeredCard$CardPower`,
/// `Sacrificed$CardManaCost`, `Remembered$CardManaCost`) and the count of opponents; `None` for any other expression.
fn event_amount(ctx: &Ctx, expression: &str) -> Tr<Option<Value>> {
    let Some((head, what)) = expression.split_once('$') else {
        return Ok(None);
    };
    let of = match head {
        "TriggerCount" => {
            return match what {
                "DamageAmount" | "LifeAmount" | "Amount" => Ok(Some(json!("Triggering"))),
                other => unmapped(format!("amount:TriggerCount${other}")),
            };
        }
        "PlayerCountOpponents" | "PlayerCountPlayers" => {
            let who = if head == "PlayerCountPlayers" {
                "EachPlayer"
            } else {
                "Opponents"
            };
            if let Some(found) = super::amount_more::highest(ctx, who, what) {
                return found.map(Some);
            }
            let key = match what {
                "Amount" => "Players",
                "HighestLifeTotal" => "HighestLife",
                "LowestLifeTotal" => "LowestLife",
                "LowestCardsInHand" => "LowestHand",
                other => return unmapped(format!("amount:{head}${other}")),
            };
            return Ok(Some(json!({ key: who })));
        }
        "PlayerCountPropertyYou" => {
            return match super::amount_more::player_amount(head, what, &json!("You")) {
                Some(found) => found.map(Some),
                None => unmapped(format!("amount:{head}${what}")),
            };
        }
        "TargetedPlayer" => {
            return match super::amount_more::player_amount(head, what, &ctx.player_target()?) {
                Some(found) => found.map(Some),
                None => unmapped(format!("amount:{head}${what}")),
            };
        }
        "TriggeredPlayer" => {
            let who = super::triggered::resolve(ctx.trigger.as_ref(), "TriggeredPlayer")?;
            return match super::amount_more::player_amount("TargetedPlayer", what, &who) {
                Some(found) => found.map(Some),
                None => unmapped(format!("amount:{head}${what}")),
            };
        }
        "TriggeredCard" => super::triggered::resolve(ctx.trigger.as_ref(), "TriggeredCard")?,
        "Sacrificed" => json!("Paid"),
        "Remembered" | "RememberedLKI" if what != "Amount" => match &ctx.remembered {
            Some(remembered) => remembered.clone(),
            None => return unmapped("amount:Remembered-without-a-change"),
        },
        _ => return Ok(None),
    };
    targeted_amount(what, &of).map(Some)
}

/// A stat of the latest target.
fn targeted_amount(what: &str, of: &Value) -> Tr<Value> {
    let stat = match what {
        "CardPower" => "Power",
        "CardToughness" => "Toughness",
        "CardManaCost" => "ManaValue",
        other => return unmapped(format!("amount:Targeted${other}")),
    };
    Ok(json!({"Stat": {"of": of, "stat": stat}}))
}

/// The amount of a `Count$` expression: its base and the `/Plus.N`, `/Minus.N`, `/Twice` or `/Times.N` steps after it.
fn count_amount(ctx: &Ctx, expression: &str) -> Tr<Value> {
    let (first, steps) = expression.split_once('/').unwrap_or((expression, ""));
    apply_steps(ctx, count_base(ctx, first)?, steps)
}

/// The `/Plus.N`, `/Minus.N`, `/Twice` and `/Times.N` steps that follow the base of an expression.
fn apply_steps(ctx: &Ctx, mut value: Value, steps: &str) -> Tr<Value> {
    for step in steps.split('/').filter(|step| !step.is_empty()) {
        value = match step.split_once('.') {
            Some(("Plus", n)) => plus(ctx, &value, n, false)?,
            Some(("Minus", n)) => plus(ctx, &value, n, true)?,
            Some(("Times", n)) => times(&value, parse_number(n)?),
            Some(("NMinus", n)) => json!({"Sum": [amount(ctx, n)?, times(&value, -1)]}),
            Some(("LimitMax", n)) => limit(ctx, &value, n, "Gt")?,
            Some(("LimitMin", n)) => limit(ctx, &value, n, "Lt")?,
            None if step == "Twice" => times(&value, 2),
            None if step == "HalfUp" => json!({"Half": {"of": value, "up": true}}),
            None if step == "HalfDown" => json!({"Half": {"of": value, "up": false}}),
            _ => return unmapped(format!("amount:Count/{}", head(step))),
        };
    }
    Ok(value)
}

/// The amount held to the bound: `LimitMax` (`Gt`) caps it, `LimitMin` (`Lt`) raises it.
fn limit(ctx: &Ctx, value: &Value, bound: &str, beyond: &str) -> Tr<Value> {
    let bound = amount(ctx, bound)?;
    let past = json!({"Compare": {"left": value, "op": beyond, "right": bound}});
    Ok(json!({"When": {"condition": past, "then": bound, "otherwise": value}}))
}

fn parse_number(text: &str) -> Tr<i32> {
    text.parse().or_else(|_| unmapped("amount:step-number"))
}

/// The amount plus (or minus) the step operand: a number, or the name of another part whose amount is added.
fn plus(ctx: &Ctx, value: &Value, operand: &str, minus: bool) -> Tr<Value> {
    if let Ok(n) = operand.parse::<i32>() {
        return Ok(sum(value, if minus { -n } else { n }));
    }
    let other = amount(ctx, operand)?;
    let other = if minus { times(&other, -1) } else { other };
    Ok(json!({"Sum": [value, other]}))
}

fn sum(value: &Value, n: i32) -> Value {
    if let Some(number) = value.as_i64().and_then(|v| i32::try_from(v).ok()) {
        return json!(number + n);
    }
    json!({"Sum": [value, n]})
}

fn times(value: &Value, n: i32) -> Value {
    json!({"Times": {"by": n, "of": value}})
}

/// The base of a `Count$` expression.
fn count_base(ctx: &Ctx, base: &str) -> Tr<Value> {
    if let Some(chosen) = super::amount_when::when(ctx, base) {
        return chosen;
    }
    if let Some(found) = super::amount_more::count(ctx, base) {
        return found;
    }
    let word = base.split_once(' ').map_or(base, |(word, _)| word);
    match word {
        "xPaid" => Ok(json!("X")),
        "CardPower" => Ok(stat_of_this("Power")),
        "CardToughness" => Ok(stat_of_this("Toughness")),
        "CardManaCost" => Ok(stat_of_this("ManaValue")),
        "YourLifeTotal" => Ok(json!({"Life": "You"})),
        "LifeYouGainedThisTurn" => Ok(tallied("You", "LifeGainedTotal")),
        "AttackersDeclared" => Ok(tallied("You", "Attackers")),
        "YouDrewThisTurn" => Ok(tallied("You", "Drawn")),
        "LifeOppsLostThisTurn" => Ok(tallied("Opponents", "LifeLost")),
        "OppLifeTotal" | "TargetedPlayerLife" => unmapped(format!("amount:Count${word}")),
        "TimesKicked" => Ok(json!("Kicks")),
        "PlayerCountOpponents" => Ok(json!({"Players": "Opponents"})),
        "PlayerCountPlayers" => Ok(json!({"Players": "EachPlayer"})),
        other if other.starts_with("ThisTurnCast_") => this_turn_cast(ctx, other),
        other => counters_or_gap(ctx, other),
    }
}

fn tallied(who: &str, what: &str) -> Value {
    json!({"Tallied": {"who": who, "what": what}})
}

fn stat_of_this(stat: &str) -> Value {
    json!({"Stat": {"of": "This", "stat": stat}})
}

/// `CardCounters.P1P1` (counters on this card), or the gap that names the base.
fn counters_or_gap(ctx: &Ctx, word: &str) -> Tr<Value> {
    if let Some(kind) = word.strip_prefix("CardCounters.").and_then(counter_kind) {
        let on = defined(ctx, "Self")?;
        return Ok(json!({"Counters": {"on": on, "counter": kind}}));
    }
    unmapped(format!("amount:Count${}", head(word)))
}

/// `ThisTurnCast_Card.YouCtrl`: the spells cast this turn, by any player, that the valid-string selects as they were cast.
fn this_turn_cast(ctx: &Ctx, word: &str) -> Tr<Value> {
    let valid = word.strip_prefix("ThisTurnCast_").unwrap_or_default();
    let filter = selector_in(ctx, valid, None)?;
    Ok(json!({"Tally": {"event": "SpellCast", "who": "EachPlayer", "filter": filter}}))
}

#[cfg(test)]
mod tests {
    //! Goal: only counter kinds the engine gives an effect map; power/toughness counters are named for their numbers and "all" stays a gap.
    use rstest::rstest;
    use serde_json::json;

    use super::counter_kind;

    #[rstest]
    #[case("P1P1", Some(json!("+1/+1")))]
    #[case("M1M1", Some(json!("-1/-1")))]
    #[case("CHARGE", Some(json!("charge")))]
    #[case("TIME", Some(json!({"Other": "time"})))]
    #[case("P2P2", Some(json!({"Other": "+2/+2"})))]
    #[case("M0M1", Some(json!({"Other": "-0/-1"})))]
    #[case("P1P0", Some(json!({"Other": "+1/+0"})))]
    #[case("P12M3", Some(json!({"Other": "+12/-3"})))]
    #[case("ALL", None)]
    fn counter_kinds(#[case] word: &str, #[case] kind: Option<serde_json::Value>) {
        assert_eq!(counter_kind(word), kind);
    }
}
