//! Effects that reveal cards of the player's hand and choose a source of damage.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, only, unmapped};
use super::valid::{defined, selector_any, selector_in};
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

/// `Reveal` with `Random$ True`: one card of the hand of the player `Defined$` names, picked at random and shown.
fn reveal_at_random(ctx: &mut Ctx, p: &Params, target: Option<Value>) -> Tr<Vec<Value>> {
    if [
        "RevealValid",
        "RevealDefined",
        "AnyNumber",
        "Optional",
        "NumCards",
    ]
    .iter()
    .any(|key| p.has(key))
    {
        return unmapped("key:Reveal.Random+shape");
    }
    // "Look at a card at random in target player's hand" shows the card to its controller alone, which a reveal does not.
    if ctx
        .oracle
        .to_lowercase()
        .contains("look at a card at random")
    {
        return unmapped("key:Reveal.Random-look");
    }
    let who = match target {
        Some(target) => target,
        None => defined(ctx, p.get("Defined").unwrap_or("You"))?,
    };
    // One card among every hand would not be one player's card.
    if matches!(who.as_str(), Some("EachPlayer" | "Opponents")) {
        return unmapped("key:Reveal.Random+group");
    }
    let bind = ctx.fresh("revealed");
    if p.get("RememberRevealed") == Some("True") {
        ctx.remembered = Some(json!({ "Bound": bind }));
    }
    Ok(vec![
        json!({"ChooseAtRandom": {"from": {"Each": [{"OwnedBy": who}, {"In": "Hand"}]}, "as": bind}}),
        json!({"Reveal": {"what": {"Bound": bind}}}),
    ])
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
            "Random",
            "NumCards",
            "ValidTgts",
        ],
    )?;
    // "Target player reveals ... from their hand": the target reveals, and picks what to reveal.
    let target = match p.get("ValidTgts") {
        None => None,
        Some("Player" | "Opponent" | "Player.Opponent")
            if !p.has("Defined") && !p.has("TargetMin") && !p.has("TargetMax") =>
        {
            Some(super::valid::subject(ctx, p, "You", None)?)
        }
        Some(_) => return unmapped("key:Reveal.ValidTgts"),
    };
    if p.get("Random") == Some("True") {
        return reveal_at_random(ctx, p, target);
    }
    if p.get("Defined").is_some_and(|who| who != "You") {
        return unmapped("key:Reveal.Defined");
    }
    // "Target opponent chooses cards from their hand. Look at those cards": only the controller sees them.
    if target.is_some() && ctx.oracle.to_lowercase().contains("look at those cards") {
        return unmapped("key:Reveal.ValidTgts-look");
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
    let spec = p.get("RevealValid").unwrap_or("Card");
    let most = amount(ctx, p.get("NumCards").unwrap_or("1"))?;
    let mut filter = selector_in(ctx, spec, Some("Hand"))?;
    let owned = filter.iter().any(|pred| pred.get("OwnedBy").is_some());
    if target.is_some() && owned {
        return unmapped("key:Reveal.ValidTgts-with-an-owner");
    }
    if !owned {
        filter.push(json!({"OwnedBy": target.clone().unwrap_or_else(|| json!("You"))}));
    }
    let count = match (
        p.get("AnyNumber") == Some("True"),
        p.get("Optional") == Some("True"),
    ) {
        (true, _) => json!({"min": 0, "max": null}),
        (false, true) => json!({"min": 0, "max": most}),
        (false, false) => json!({"min": most, "max": most}),
    };
    let bind = ctx.fresh("revealed");
    if remember {
        ctx.remembered = Some(json!({ "Bound": bind }));
    }
    let mut choose = json!({"Choose": {"from": {"Each": filter}, "count": count, "as": bind}});
    if let Some(target) = target {
        choose["Choose"]["who"] = target;
    }
    Ok(vec![choose, json!({"Reveal": {"what": {"Bound": bind}}})])
}
