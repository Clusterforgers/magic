//! More cost words: counters put on or removed from other permanents, exert, damage, blight, evidence, exile from the top
//! of the library, beholding, forage and untapping permanents.

use serde_json::{Value, json};

use super::amount::{amount, counter_kind};
use super::ctx::{Ctx, Tr, unmapped};
use super::valid::selector_any;

/// A cost word without brackets.
pub fn bare(ctx: &Ctx, word: &str) -> Option<Value> {
    match word {
        "Forage" => Some(json!("Forage")),
        // The card's own mana cost, read off each card a static ability gives the keyword to.
        "CardManaCost" if ctx.own_cost_ok.get() => Some(json!("OwnManaCost")),
        _ => None,
    }
}

/// The cost item of `head<fields>` when this module reads the head; `None` for any other head.
pub fn item(ctx: &Ctx, head: &str, fields: &[&str]) -> Option<Tr<Value>> {
    let number = fields.first().copied().unwrap_or_default();
    let what = fields.get(1).copied().unwrap_or_default();
    let on_self = matches!(what, "CARDNAME" | "NICKNAME" | "Card.Self" | "");
    Some(match head {
        "Exert" if on_self && number == "1" => Ok(json!("Exert")),
        "Blight" => amount(ctx, number).map(|n| json!({ "Blight": n })),
        "Draw" if what == "You" => fixed(number).map(|n| json!({ "Draw": n })),
        "DamageYou" => amount(ctx, number).map(|n| json!({ "DamageToYou": n })),
        "ExileFromTop" if what == "Card" => amount(ctx, number).map(|n| json!({ "ExileTop": n })),
        "CollectEvidence" => match number.parse::<u32>() {
            Ok(n) => Ok(json!({ "CollectEvidence": n })),
            Err(_) => unmapped("cost:CollectEvidence-X"),
        },
        "Behold" => filtered(ctx, "Behold", number, what),
        "Waterbend" => match number.parse::<u32>() {
            Ok(n) if n > 0 && ctx.waterbend_ok.get() => Ok(json!({ "Waterbend": n })),
            Ok(_) => unmapped("cost:Waterbend-outside-an-ability"),
            Err(_) => unmapped("cost:Waterbend-X"),
        },
        // Only an effect that resolves links the card it exiles to its source; a replacement ("exile it instead") does not.
        "ExiledMoveToGrave"
            if what.contains("ExiledWithSource")
                && ctx.oracle.to_lowercase().contains("exile it instead") =>
        {
            unmapped("cost:ExiledMoveToGrave-linked-by-a-replacement")
        }
        "ExiledMoveToGrave" => filtered(ctx, "ExiledToGraveyard", number, what),
        "untapYType" => filtered(ctx, "UntapPermanents", number, what),
        "AddCounter" if fields.len() == 2 => put_counters(ctx, number, what),
        "SubCounter" if fields.len() >= 3 => remove_from(ctx, number, what, fields[2]),
        "RemoveAnyCounter" => remove_among(ctx, number, what, fields.get(2).copied()),
        _ => return None,
    })
}

/// Discarding cards at random: the engine picks them from the hand with its random source.
pub fn random(number: &str) -> Tr<Value> {
    fixed(number).map(|n| json!({ "DiscardRandom": n }))
}

/// `Cost$ Mandatory Sac<1/CARDNAME>`: the source is sacrificed without a choice, and the effects follow if it was
/// ("Sacrifice it. If you do, ...").
pub fn mandatory(cost: &str, effects: &[Value]) -> Tr<Vec<Value>> {
    if !matches!(
        cost,
        "Sac<1/CARDNAME>" | "Sac<1/CARDNAME/this artifact>" | "Sac<1/CARDNAME/this creature>"
    ) {
        return unmapped("cost:Mandatory");
    }
    Ok(vec![
        json!({"Sacrifice": {"filter": ["This"]}}),
        json!({"If": {"condition": "Performed", "then": effects}}),
    ])
}

/// A fixed count: a counted draw or discard is paid by the engine only for a number.
fn fixed(number: &str) -> Tr<u32> {
    match number.parse::<u32>() {
        Ok(n) if n > 0 => Ok(n),
        _ => unmapped("cost:counted-draw-or-discard"),
    }
}

fn filtered(ctx: &Ctx, variant: &str, number: &str, what: &str) -> Tr<Value> {
    Ok(json!({ variant: {"filter": selector_any(ctx, what)?, "count": amount(ctx, number)?} }))
}

fn put_counters(ctx: &Ctx, number: &str, kind: &str) -> Tr<Value> {
    let Some(kind) = counter_kind(kind) else {
        return unmapped("cost:AddCounter-kind");
    };
    Ok(json!({"PutCounters": {"counter": kind, "count": amount(ctx, number)?}}))
}

/// One counter from one permanent the payer controls that matches.
fn remove_from(ctx: &Ctx, number: &str, kind: &str, from: &str) -> Tr<Value> {
    let Some(counter) = counter_kind(kind) else {
        return unmapped("cost:SubCounter-kind");
    };
    if matches!(from, "CARDNAME" | "NICKNAME") {
        return Ok(json!({"RemoveCounters": {"counter": counter, "count": amount(ctx, number)?}}));
    }
    if number != "1" {
        return unmapped("cost:SubCounter-from-count");
    }
    Ok(
        json!({"RemoveCounters": {"counter": counter, "count": 1, "from": selector_any(ctx, from)?}}),
    )
}

/// Counters from among permanents (`Any` kind: the payer picks): the source itself when `from` names it.
fn remove_among(ctx: &Ctx, number: &str, kind: &str, from: Option<&str>) -> Tr<Value> {
    let counter = if kind == "Any" {
        None
    } else {
        match counter_kind(kind) {
            Some(counter) => Some(counter),
            None => return unmapped("cost:RemoveAnyCounter-kind"),
        }
    };
    let count = amount(ctx, number)?;
    match from {
        Some("CARDNAME" | "NICKNAME") => {
            let mut cost = json!({"RemoveCounters": {"count": count}});
            if let Some(counter) = counter {
                cost["RemoveCounters"]["counter"] = counter;
            }
            Ok(cost)
        }
        Some(valid) => {
            let mut cost =
                json!({"RemoveCountersAmong": {"count": count, "from": selector_any(ctx, valid)?}});
            if let Some(counter) = counter {
                cost["RemoveCountersAmong"]["counter"] = counter;
            }
            Ok(cost)
        }
        None => unmapped("cost:RemoveAnyCounter-from"),
    }
}
