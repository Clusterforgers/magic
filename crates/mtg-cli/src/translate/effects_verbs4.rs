//! Effects that reveal cards of the player's hand and choose a source of damage.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, only, unmapped};
use super::valid::{selector_any, selector_in};
use crate::forge::script::Params;

/// The effects of a verb this module reads; `None` for any other API.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Option<Tr<Vec<Value>>> {
    match api {
        "Reveal" => Some(reveal(ctx, p)),
        "ChooseSource" => Some(choose_source(ctx, p)),
        _ => None,
    }
}

/// `ChooseSource`: the controller picks a source of damage, an object on the battlefield or the stack or in a command
/// zone (an emblem), that the choices allow; the later lines read it as `ChosenCard` (a shield that holds the chosen
/// object). With no legal source nothing is chosen and the shield names nothing. A chooser other than the controller,
/// remembering the choice and an unless cost stay gaps.
fn choose_source(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "ChooseSource", &["Choices", "ChoiceTitle"])?;
    let mut filter = vec![json!({"AnyOf": [
        {"In": "Battlefield"}, {"In": "Stack"}, {"In": "Command"}
    ]})];
    filter.extend(source_restriction(
        ctx,
        p.get("Choices").unwrap_or("Card,Emblem"),
    )?);
    let bind = ctx.fresh("source");
    ctx.chosen_card = Some(bind.clone());
    Ok(vec![
        json!({"Choose": {"from": {"Each": filter}, "as": bind}}),
    ])
}

/// What the `Choices$` of a source choice require of the source beyond being a card or an emblem: nothing, one
/// requirement shared by every alternative, or one of several single requirements ("a black or red source").
fn source_restriction(ctx: &Ctx, choices: &str) -> Tr<Vec<Value>> {
    let mut sets: Vec<Vec<Value>> = Vec::new();
    let mut open = false;
    for alternative in choices.split(',').map(str::trim) {
        let tail = match alternative.split_once('.') {
            None if matches!(alternative, "Card" | "Emblem") => "",
            None => alternative,
            Some(("Card" | "Emblem", tail)) => tail,
            Some(_) => return unmapped("key:ChooseSource.Choices"),
        };
        if tail.is_empty() {
            open = true;
            continue;
        }
        let spec = tail
            .split('+')
            .map(|part| match part.strip_suffix("Source") {
                Some(color) if !color.is_empty() => color.to_owned(),
                _ => part.to_owned(),
            })
            .collect::<Vec<_>>()
            .join("+");
        let preds = selector_any(ctx, &format!("Card.{spec}"))?;
        if !sets.contains(&preds) {
            sets.push(preds);
        }
    }
    match (open, sets.len()) {
        (_, 0) => Ok(Vec::new()),
        (true, _) => unmapped("key:ChooseSource.Choices-open-and-restricted"),
        (false, 1) => Ok(sets.remove(0)),
        (false, _) if sets.iter().all(|preds| preds.len() == 1) => Ok(vec![
            json!({"AnyOf": sets.into_iter().flatten().collect::<Vec<_>>()}),
        ]),
        _ => unmapped("key:ChooseSource.Choices-alternatives"),
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
