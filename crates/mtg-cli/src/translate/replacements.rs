//! Replacement effects: an `R:` line as the DSL's `Replacement` ability (a permanent that enters tapped or with counters).

use serde_json::{Value, json};

use super::amount::{amount, counter_kind};
use super::ctx::{Ctx, Tr, is_hint, unmapped};
use crate::forge::script::Params;

/// A replacement line as a `Replacement` ability.
pub fn replacement_line(ctx: &mut Ctx, body: &str) -> Tr<Value> {
    ctx.reset();
    let p = Params::parse(body);
    let event = p.get("Event").unwrap_or_default();
    if event != "Moved" {
        return unmapped(format!("replacement:{event}"));
    }
    for (key, _) in p.iter() {
        let read = ["Event", "ValidCard", "Destination", "ReplaceWith", "ReplacementResult"];
        if !read.contains(&key) && !is_hint(key) {
            return unmapped(format!("key:Replacement.Moved.{key}"));
        }
    }
    if p.get("ValidCard") != Some("Card.Self") || p.get("Destination") != Some("Battlefield") {
        return unmapped("replacement:Moved-shape");
    }
    let Some(name) = p.get("ReplaceWith") else {
        return unmapped("replacement:Moved-without-ReplaceWith");
    };
    let part = ctx.sub(name)?;
    let by = enters_by(ctx, &part)?;
    Ok(json!({"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": by}}))
}

/// What the permanent does as it enters, from the part a replacement names.
fn enters_by(ctx: &Ctx, part: &Params) -> Tr<Value> {
    let api = part.get("DB").unwrap_or_default();
    if part.get("Defined") != Some("Self") || part.get("ETB") != Some("True") {
        return unmapped("replacement:Moved-not-an-entering-effect");
    }
    match api {
        "Tap" => {
            only_keys(part, &["DB", "Defined", "ETB"])?;
            Ok(json!("Tapped"))
        }
        "PutCounter" => {
            only_keys(part, &["DB", "Defined", "ETB", "CounterType", "CounterNum"])?;
            let Some(kind) = part.get("CounterType").and_then(counter_kind) else {
                return unmapped("key:Replacement.CounterType");
            };
            let count = amount(ctx, part.get("CounterNum").unwrap_or("1"))?;
            Ok(json!({"WithCounters": {"counter": kind, "count": count}}))
        }
        other => unmapped(format!("replacement:Moved-{other}")),
    }
}

fn only_keys(part: &Params, read: &[&str]) -> Tr<()> {
    for (key, _) in part.iter() {
        if !read.contains(&key) && !is_hint(key) {
            return unmapped(format!("key:Replacement.{key}"));
        }
    }
    Ok(())
}
