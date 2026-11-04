//! `MayPlay$ True`: permission to cast or play cards from another zone, as the DSL's `MayCast` rule, and the `Play` effect.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, only, unmapped};
use super::valid::{defined, selector_any, selector_in};
use crate::forge::script::Params;

/// The keys of a `MayPlay` static line.
const READ: [&str; 13] = [
    "RaiseCost",
    "Mode",
    "MayPlay",
    "Affected",
    "AffectedZone",
    "MayPlayLimit",
    "MayPlayWithoutManaCost",
    "MayPlayWithFlash",
    "EffectZone",
    "Description",
    "MayPlayPlayer",
    "MayPlayText",
    "Secondary",
];

/// The `MayCast` rule of a static line. `only` names the objects it covers (what an `Effect` object remembers); without
/// it the cards `Affected$` selects are covered.
pub fn may_cast(ctx: &Ctx, p: &Params, only: Option<&Value>) -> Tr<Value> {
    check_keys(p, "Static.MayPlay", &READ, |_, _| false)?;
    let from = match p.get("AffectedZone") {
        Some(zone @ ("Graveyard" | "Exile" | "Library" | "Hand")) => zone,
        Some(other) => return unmapped(format!("zone:MayPlay.AffectedZone={other}")),
        None => return unmapped("key:Static.MayPlay.no-AffectedZone"),
    };
    if !matches!(p.get("EffectZone"), None | Some("Battlefield")) && only.is_none() {
        return unmapped("key:Static.MayPlay.EffectZone");
    }
    let mut permission = json!({ "from": from });
    match (&only, p.get("Affected").unwrap_or("Card")) {
        (Some(objects), "Card.IsRemembered" | "Card.IsRemembered+YouOwn") => {
            permission["only"] = (*objects).clone();
            // Exile holds anyone's cards: "the card you own" excludes the others among the ones named.
            if p.get("Affected")
                .is_some_and(|valid| valid.ends_with("+YouOwn"))
            {
                permission["what"] = json!([{"OwnedBy": "You"}]);
            }
        }
        (Some(_), _) => return unmapped("valid:MayPlay-Affected-with-remembered"),
        (None, valid) => {
            // A graveyard, hand or library is the player's own: the ownership is in `from`. Exile holds anyone's cards.
            let what: Vec<Value> = selector_any(ctx, valid)?
                .into_iter()
                .filter(|pred| from == "Exile" || *pred != json!({"OwnedBy": "You"}))
                .filter(|pred| *pred != json!({"ControlledBy": "You"}))
                .collect();
            if !what.is_empty() {
                permission["what"] = json!(what);
            }
        }
    }
    if p.get("MayPlayWithoutManaCost") == Some("True") {
        permission["free"] = json!(true);
    }
    if p.get("MayPlayWithFlash") == Some("True") {
        permission["flash"] = json!(true);
    }
    // What the caster pays in addition to the other costs of the cast.
    if let Some(extra) = p.get("RaiseCost") {
        permission["additional"] = super::cost::cost(ctx, extra)?;
    }
    match p.get("MayPlayLimit") {
        None => {}
        Some("1") => permission["once"] = json!(true),
        Some(other) => return unmapped(format!("key:Static.MayPlay.MayPlayLimit={other}")),
    }
    match p.get("MayPlayPlayer") {
        None | Some("You") => {}
        Some(other) => return unmapped(format!("key:Static.MayPlay.MayPlayPlayer={other}")),
    }
    // The label of one line of a per-type permission (Muldrotha): each line holds its own `Affected$` and limit.
    let label = p.get("MayPlayText").unwrap_or_default().to_lowercase();
    let text = p.get("Description").unwrap_or_default().to_lowercase();
    if text.contains("play") || label == "land" {
        permission["play"] = json!(true);
    }
    Ok(json!({ "MayCast": permission }))
}

/// How many of the cards a `Play` casts.
enum Count {
    One,
    Some(u64),
    All,
}

/// `Play`: the player may cast (or play) the cards `Defined$` names or `Valid$` selects in `ValidZone$`, up to `Amount$` of
/// them, for free when `WithoutManaCost$` says so; `CopyCard$` casts a copy of each card in exile.
pub fn play(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Play",
        &[
            "Defined",
            "Valid",
            "ValidZone",
            "Optional",
            "WithoutManaCost",
            "ValidSA",
            "Amount",
            "Controller",
            "CopyCard",
        ],
    )?;
    if p.get("Optional") != Some("True") {
        return unmapped("key:Play.Optional");
    }
    let count = match p.get("Amount") {
        None | Some("1") => Count::One,
        Some("All") => Count::All,
        Some(n) => match n.parse::<u64>() {
            Ok(n) if n >= 2 => Count::Some(n),
            _ => return unmapped("key:Play.Amount"),
        },
    };
    let who = match p.get("Controller") {
        None | Some("You") => json!("You"),
        Some(word) => defined(ctx, word)?,
    };
    let spell_only = match p.get("ValidSA") {
        None => false,
        Some("Spell") => true,
        Some(_) => return unmapped("key:Play.ValidSA"),
    };
    let copy = match p.get("CopyCard") {
        None => false,
        Some("True") if p.get("ValidZone") == Some("Exile") => true,
        Some(_) => return unmapped("key:Play.CopyCard"),
    };
    let pool = match (p.get("Defined"), p.get("Valid"), p.get("ValidZone")) {
        (Some(word), None, None) => defined(ctx, word)?,
        (None, Some(valid), Some(zone)) => {
            json!({ "Each": selector_in(ctx, valid, Some(zone))? })
        }
        (None, Some(_), None) => return unmapped("key:Play.no-ValidZone"),
        (None, None, _) => return unmapped("key:Play.no-Defined"),
        _ => return unmapped("key:Play.Defined-with-Valid"),
    };
    let free = p.get("WithoutManaCost") == Some("True");
    let cast = |card: Value, ctx: &mut Ctx| -> Vec<Value> {
        let mut steps = Vec::new();
        let mut card = card;
        if copy {
            let made = ctx.fresh("copy");
            steps.push(json!({"CopyCard": {"who": who, "what": card, "as": made}}));
            card = json!({ "Bound": made });
        }
        let mut effect = json!({"who": who, "what": card});
        if free {
            effect["free"] = json!(true);
        }
        if !spell_only {
            effect["play"] = json!(true);
        }
        steps.push(json!({ "CastFromZone": effect }));
        steps
    };
    if matches!(count, Count::One) && p.has("Defined") {
        return Ok(cast(pool, ctx));
    }
    let (over, mut steps) = match count {
        Count::All => (pool, Vec::new()),
        Count::One | Count::Some(_) => {
            let max = if let Count::Some(n) = count { n } else { 1 };
            let picked = ctx.fresh("picked");
            let choose = json!({"Choose": {"who": who, "from": pool,
                "count": {"min": 0, "max": max}, "as": picked}});
            (json!({ "Bound": picked }), vec![choose])
        }
    };
    if matches!(count, Count::One) {
        steps.extend(cast(over, ctx));
        return Ok(steps);
    }
    let each = ctx.fresh("card");
    let inner = cast(json!({ "Bound": each }), ctx);
    steps.push(json!({"RepeatEach": {"over": over, "as": each, "effects": inner}}));
    Ok(steps)
}
