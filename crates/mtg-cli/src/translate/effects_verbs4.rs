//! Effects that reveal cards of the player's hand and choose a source of damage.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, only, unmapped};
use super::valid::selector_in;
use crate::forge::script::Params;

/// The effects of a verb this module reads; `None` for any other API.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Option<Tr<Vec<Value>>> {
    match api {
        "Reveal" => Some(reveal(ctx, p)),
        _ => None,
    }
}

/// `Reveal`: the card itself (`RevealDefined$ Self`), or cards of the player's own hand that `RevealValid$` allows (any
/// number, up to one when optional, else one), named for the lines that read what was revealed. Revealing at random,
/// the hand of another player, all matching cards or a set of cards other than these stay gaps.
fn reveal(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Reveal",
        &[
            "Defined",
            "RevealDefined",
            "RevealValid",
            "AnyNumber",
            "Optional",
            "RememberRevealed",
        ],
    )?;
    if p.get("Defined").is_some_and(|who| who != "You") {
        return unmapped("key:Reveal.Defined");
    }
    let remember = p.get("RememberRevealed") == Some("True");
    if p.get("RevealDefined") == Some("Self") {
        if p.has("RevealValid") || p.has("AnyNumber") || p.has("Optional") || remember {
            return unmapped("key:Reveal.RevealDefined+other");
        }
        return Ok(vec![json!({"Reveal": {"what": "This"}})]);
    }
    if p.has("RevealDefined") {
        return unmapped("key:Reveal.RevealDefined");
    }
    let Some(spec) = p.get("RevealValid") else {
        return unmapped("key:Reveal.NumCards");
    };
    let mut filter = selector_in(ctx, spec, Some("Hand"))?;
    if !filter.iter().any(|pred| pred.get("OwnedBy").is_some()) {
        filter.push(json!({"OwnedBy": "You"}));
    }
    let count = match (
        p.get("AnyNumber") == Some("True"),
        p.get("Optional") == Some("True"),
    ) {
        (true, _) => json!({"min": 0, "max": null}),
        (false, true) => json!({"min": 0, "max": 1}),
        (false, false) => json!({"min": 1, "max": 1}),
    };
    let bind = ctx.fresh("revealed");
    if remember {
        ctx.remembered = Some(json!({ "Bound": bind }));
    }
    Ok(vec![
        json!({"Choose": {"from": {"Each": filter}, "count": count, "as": bind}}),
        json!({"Reveal": {"what": {"Bound": bind}}}),
    ])
}
