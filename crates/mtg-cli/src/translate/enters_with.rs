//! `K:ETBReplacement:Other:<part>:Mandatory::Card.Self+<how>` (CR 614.1c): "If it was kicked, it enters with two +1/+1
//! counters on it and with first strike". The part chain puts the counters and animates the permanent for good.

use serde_json::{Value, json};

use crate::translate::amount::{amount, counter_kind};
use crate::translate::cost::cost;
use crate::translate::ctx::{Ctx, Tr, check_keys, unmapped};
use crate::translate::keyword::keyword_list;

/// How many parts of the chain are followed; a longer chain is a gap.
const LONGEST: usize = 4;

/// The condition the valid-string names: kicked (with its first or second kicker) or escaped.
fn condition(ctx: &Ctx, valid: &str) -> Tr<Value> {
    let Some(how) = valid.strip_prefix("Card.Self+") else {
        return unmapped("keyword:ETBReplacement-shape");
    };
    match how.split_once(' ') {
        None if how == "kicked" => Ok(json!("Kicked")),
        None if how == "escaped" => Ok(json!({"CastWith": "Escape"})),
        Some(("kicked", number)) => {
            let Ok(at) = number.parse::<usize>() else {
                return unmapped("keyword:ETBReplacement-kicker-number");
            };
            let kickers: Vec<&str> = ctx
                .keywords
                .iter()
                .filter_map(|line| line.strip_prefix("Kicker:"))
                .flat_map(|line| line.split(':').take(2))
                .collect();
            match kickers.get(at.wrapping_sub(1)) {
                Some(text) if kickers.len() > 1 => Ok(json!({"KickedWith": cost(ctx, text)?})),
                _ => unmapped("keyword:ETBReplacement-kicker-number"),
            }
        }
        _ => unmapped("keyword:ETBReplacement-shape"),
    }
}

/// The replacement of a permanent that enters with the counters and the changes the chain of parts names.
pub fn self_enters(ctx: &mut Ctx, name: &str, valid: &str) -> Tr<Value> {
    let when = condition(ctx, valid)?;
    let mut counter: Option<(Value, Value)> = None;
    let mut mods: Vec<Value> = Vec::new();
    let mut next = Some(name.to_owned());
    let mut seen = 0;
    while let Some(part) = next.take() {
        seen += 1;
        if seen > LONGEST {
            return unmapped("keyword:ETBReplacement-chain");
        }
        let p = ctx.sub(&part)?;
        check_keys(
            &p,
            "ETBReplacement",
            &[
                "DB",
                "ETB",
                "Defined",
                "CounterType",
                "CounterNum",
                "SubAbility",
                "Keywords",
                "Duration",
                "Triggers",
                "Abilities",
            ],
            |_, _| false,
        )?;
        if !matches!(p.get("Defined"), None | Some("Self")) {
            return unmapped("keyword:ETBReplacement-shape");
        }
        match p.get("DB") {
            Some("PutCounter") if p.get("ETB") == Some("True") && counter.is_none() => {
                let Some(kind) = p.get("CounterType").and_then(counter_kind) else {
                    return unmapped("keyword:ETBReplacement-counter-kind");
                };
                counter = Some((
                    json!(kind),
                    amount(ctx, p.get("CounterNum").unwrap_or("1"))?,
                ));
            }
            Some("Animate") if p.get("Duration") == Some("Permanent") => {
                if let Some(words) = p.get("Keywords") {
                    for keyword in keyword_list(ctx, words)? {
                        mods.push(json!({ "AddKeyword": keyword }));
                    }
                }
                mods.extend(crate::translate::effects_animate::abilities(ctx, &p)?);
            }
            _ => return unmapped("keyword:ETBReplacement-shape"),
        }
        next = p.get("SubAbility").map(str::to_owned);
    }
    // The cards an escape exiles as a cost are not linked to the permanent, so a granted ability that reads them finds none.
    if when.get("CastWith").is_some() && json!(mods).to_string().contains("ExiledWith") {
        return unmapped("keyword:ETBReplacement-escape-exiled-cards");
    }
    let (kind, count) = counter.unwrap_or_else(|| (json!("+1/+1"), json!(0)));
    let by = if mods.is_empty() {
        json!({"WithCounters": {"counter": kind, "count": count}})
    } else {
        json!({"WithCountersAnd": {"counter": kind, "count": count, "mods": mods}})
    };
    Ok(json!({"Replacement": {
        "when": {"Enters": {"what": ["This"]}}, "by": by, "condition": when}}))
}
