//! A `Repeat` that its caster may go on with (`RepeatOptional$ True`, "You may repeat this process any number of
//! times"): the body once, then again each time the caster says yes, as Forge asks after each pass. A target on the
//! line (`ValidTgts$`) is the target its body names (`Defined$ Targeted`).

use mtg_cards::effect::MOST_PASSES;
use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::valid::target_spec;
use crate::forge::script::Params;

/// Who is asked before each pass after the first: the caster for `RepeatOptional$ True`, nobody without the key. Another
/// decider (`RepeatOptionalDecider$`) is not read by the mapper and stays a gap.
pub fn asker(p: &Params) -> Tr<Option<Value>> {
    match p.get("RepeatOptional") {
        None => Ok(None),
        Some("True") => Ok(Some(json!("You"))),
        Some(other) => unmapped(format!("key:Repeat.RepeatOptional={other}")),
    }
}

/// Declares the target of the line, which the body reads as `Targeted`.
pub fn target(ctx: &mut Ctx, p: &Params) -> Tr<()> {
    if p.has("ValidTgts") {
        let spec = target_spec(ctx, p, None)?;
        ctx.target(spec)?;
    }
    Ok(())
}

/// The repeat `built` asks `ask` before each pass after the first: the first pass needs no answer, as Forge resolves the
/// body before it asks.
pub fn asking(mut built: Vec<Value>, ask: Option<Value>) -> Vec<Value> {
    let Some(ask) = ask else {
        return built;
    };
    for repeat in &mut built {
        if let Some(body) = repeat.get_mut("RepeatWhile") {
            body["ask"] = ask.clone();
            body["at_least_once"] = json!(true);
        }
    }
    built
}

/// The repeat of a body that ends only when the caster stops: no check and no bound but [`MOST_PASSES`].
pub fn until_stopped(effects: Vec<Value>) -> Value {
    json!({"RepeatWhile": {
        "condition": {"Compare": {"left": 0, "op": "Eq", "right": 0}},
        "effects": effects,
        "most": MOST_PASSES,
        "at_least_once": true}})
}
