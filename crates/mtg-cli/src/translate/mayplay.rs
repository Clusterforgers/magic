//! `MayPlay$ True`: permission to cast or play cards from another zone, as the DSL's `MayCast` rule, and the `Play` effect.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, only, unmapped};
use super::valid::{defined, selector_any, selector_in};
use crate::forge::script::Params;

/// The keys of a `MayPlay` static line.
const READ: [&str; 15] = [
    "MayPlayIgnoreType",
    "MayPlayIgnoreColor",
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
    let own_zone = effect_zone(p);
    if only.is_none() && own_zone?.is_some() {
        let itself = matches!(p.get("Affected"), Some("Card.Self" | "Card.Self+nonLand"));
        if !itself || p.get("EffectZone") != p.get("AffectedZone") {
            return unmapped("key:Static.MayPlay.EffectZone");
        }
    }
    let mut permission = json!({ "from": from });
    let affected = p.get("Affected").unwrap_or("Card");
    match (&only, affected) {
        (Some(objects), valid) => {
            // "That card" with more words (`Card.IsRemembered+nonLand`): the remembered objects the rest still matches.
            let Some(props) = valid.strip_prefix("Card.") else {
                return unmapped("valid:MayPlay-Affected-with-remembered");
            };
            let parts: Vec<&str> = props.split('+').collect();
            if !parts.contains(&"IsRemembered") {
                return unmapped("valid:MayPlay-Affected-with-remembered");
            }
            permission["only"] = (*objects).clone();
            let rest: Vec<&str> = parts
                .into_iter()
                .filter(|part| *part != "IsRemembered")
                .collect();
            // Exile holds anyone's cards: "the card you own" excludes the others among the ones named.
            if !rest.is_empty() {
                let what = selector_any(ctx, &format!("Card.{}", rest.join("+")))?;
                if !what.is_empty() {
                    permission["what"] = json!(what);
                }
            }
        }
        (None, valid) => {
            // A graveyard, hand or library is the player's own: the ownership is in `from`. Exile holds anyone's cards.
            let mut what: Vec<Value> = selector_any(ctx, valid)?
                .into_iter()
                .filter(|pred| from == "Exile" || *pred != json!({"OwnedBy": "You"}))
                .filter(|pred| *pred != json!({"ControlledBy": "You"}))
                .collect();
            // A permanent card is not an instant or a sorcery.
            what.extend(super::valid::permanent_card(valid)?);
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
    // Mana of any type pays for the cast (CR 609.4b); of any color is the same to the engine.
    match (p.get("MayPlayIgnoreType"), p.get("MayPlayIgnoreColor")) {
        (Some("True"), _) => permission["spend_as"] = json!("AnyType"),
        (None, Some("True")) => permission["spend_as"] = json!("AnyColor"),
        (None, None) => {}
        _ => return unmapped("key:Static.MayPlay.MayPlayIgnore-value"),
    }
    match p.get("MayPlayPlayer") {
        None | Some("You") => {}
        // The owner of the cards named may cast them.
        Some("CardOwner") if only.is_some() => {
            permission["who"] = json!({ "OwnerOf": only });
        }
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
    let (spell_only, spell_filter) = valid_sa(ctx, p.get("ValidSA"))?;
    let copy = match p.get("CopyCard") {
        None => false,
        // A named card is copied from where it lies (the cards a move to exile remembered).
        Some("True") if p.get("ValidZone") == Some("Exile") || p.has("Defined") => true,
        Some(_) => return unmapped("key:Play.CopyCard"),
    };
    let pool = match (p.get("Defined"), p.get("Valid"), p.get("ValidZone")) {
        (Some(_), None, None) if !spell_filter.is_empty() => {
            return unmapped("key:Play.Defined-with-ValidSA-filter");
        }
        (Some(word), None, None) => defined(ctx, word)?,
        (None, Some(valid), Some(zone)) => {
            let mut selector = selector_in(ctx, valid, Some(zone))?;
            selector.extend(spell_filter);
            json!({ "Each": selector })
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

/// `ValidSA$` of a `Play`: whether only casting is allowed, and what the spell must be besides (`Spell.cmcLE4`,
/// `Instant.cmcEQX,Sorcery.cmcEQX`), as predicates of the card cast.
fn valid_sa(ctx: &Ctx, text: Option<&str>) -> Tr<(bool, Vec<Value>)> {
    let Some(text) = text else {
        return Ok((false, Vec::new()));
    };
    if text == "Spell" {
        return Ok((true, Vec::new()));
    }
    let alternatives: Vec<String> = text
        .split(',')
        .map(|part| match part.strip_prefix("Spell") {
            Some(rest) if rest.starts_with('.') => format!("Card{rest}"),
            _ => part.to_owned(),
        })
        .collect();
    let known = |a: &String| matches!(a.split('.').next(), Some("Card" | "Instant" | "Sorcery"));
    if !alternatives.iter().all(known) {
        return unmapped("key:Play.ValidSA");
    }
    Ok((true, selector_any(ctx, &alternatives.join(","))?))
}

/// The zone a `MayPlay` static works from when the card itself carries it (`EffectZone$ Graveyard`); `None` on the battlefield.
pub fn effect_zone(p: &Params) -> Tr<Option<&'static str>> {
    match p.get("EffectZone") {
        None | Some("Battlefield") => Ok(None),
        Some("Graveyard") => Ok(Some("Graveyard")),
        Some("Exile") => Ok(Some("Exile")),
        Some(_) => unmapped("key:Static.MayPlay.EffectZone"),
    }
}
