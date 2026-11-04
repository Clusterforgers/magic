//! "Can't be countered" (an `Event$ Counter` replacement with `Layer$ CantHappen`) and the replacements of an amount: life
//! gained, cards drawn and counters put.

use serde_json::{Value, json};

use super::amount::counter_kind;
use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::triggers::player_ref;
use super::valid::selector_any;
use crate::forge::script::Params;

/// A `Counter` event line: the spell itself (from the stack) or the group of spells it names (from the battlefield).
pub fn counter(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let read = ["Event", "Layer", "ValidSA", "ValidCard", "ActiveZones"];
    check_keys(p, "Replacement.Counter", &read, |_, _| false)?;
    if p.get("Layer") != Some("CantHappen") {
        return unmapped("replacement:Counter-instead");
    }
    // An ability is not a spell: a rule over abilities stays a gap.
    let sa = p.get("ValidSA").unwrap_or_default();
    if sa
        .split(',')
        .any(|one| one.split('.').next() != Some("Spell"))
    {
        return unmapped("replacement:Counter-abilities");
    }
    let zone = p.get("ActiveZones");
    if p.get("ValidCard") == Some("Card.Self") && sa == "Spell" {
        if zone.is_some() {
            return unmapped("replacement:Counter-zone");
        }
        return Ok(json!({"Rule": {"rule": {"CantBeCountered": "This"}, "from": "Stack"}}));
    }
    if zone != Some("Battlefield") {
        return unmapped("replacement:Counter-zone");
    }
    // `ValidCard` is the countered card itself: it names the group only beside a plain `Spell`.
    let spec = match (p.get("ValidCard"), sa) {
        (None, sa) => sa,
        (Some(card), "Spell") => card,
        _ => return unmapped("replacement:Counter-shape"),
    };
    let mut preds = selector_any(ctx, spec)?;
    if p.get("ValidCard").is_some() {
        preds.insert(0, json!("Spell"));
    }
    Ok(json!({"Rule": {"rule": {"CantBeCountered": {"Each": preds}}}}))
}

/// The change a `ReplaceCount$<what>/<how>` amount makes: twice, or plus a number.
fn changed_by(word: &str, what: &str) -> Tr<Value> {
    let Some(how) = word
        .strip_prefix("ReplaceCount$")
        .and_then(|rest| rest.strip_prefix(what))
        .and_then(|rest| rest.strip_prefix('/'))
    else {
        return unmapped(format!("amount:{word}"));
    };
    match how {
        "Twice" => Ok(json!({"Multiply": 2})),
        _ => match how
            .strip_prefix("Plus.")
            .and_then(|n| n.parse::<u32>().ok())
        {
            Some(n) if n > 0 => Ok(json!({"Add": n})),
            _ => unmapped(format!("amount:{word}")),
        },
    }
}

/// The amount a `ReplaceEffect` or `ReplaceCounter` part sets, which names an `SVar` holding the change.
fn part_change(ctx: &Ctx, amount: Option<&str>, what: &str) -> Tr<Value> {
    let Some(amount) = amount else {
        return unmapped("key:Replacement.no-Amount");
    };
    changed_by(ctx.svar(amount).unwrap_or(amount), what)
}

/// `Event$ GainLife`: a player gains twice that much, or that much plus a number.
pub fn gain_life(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let read = ["Event", "ValidPlayer", "ReplaceWith", "ActiveZones"];
    check_keys(p, "Replacement.GainLife", &read, |_, _| false)?;
    if p.get("ActiveZones") != Some("Battlefield") {
        return unmapped("key:Replacement.GainLife.ActiveZones");
    }
    let who = player_ref(p.get("ValidPlayer"))?;
    let Some(name) = p.get("ReplaceWith") else {
        return unmapped("replacement:GainLife-without-ReplaceWith");
    };
    let part = ctx.sub(name)?;
    let read = ["DB", "VarName", "VarValue"];
    check_keys(&part, "Replacement.GainLife", &read, |_, _| false)?;
    if part.get("DB") != Some("ReplaceEffect") || part.get("VarName") != Some("LifeGained") {
        return unmapped("replacement:GainLife-instead");
    }
    let by = part_change(ctx, part.get("VarValue"), "LifeGained")?;
    Ok(json!({"Replacement": {"when": {"LifeGain": {"who": who}}, "by": by}}))
}

/// `Event$ Draw`: a player skips the draw, or draws two instead, except the first of their draw step when the line says so.
pub fn draw(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let read = [
        "Event",
        "ValidPlayer",
        "ReplaceWith",
        "ActiveZones",
        "Prevent",
        "NotFirstCardInDrawStep",
    ];
    check_keys(p, "Replacement.Draw", &read, |_, _| false)?;
    if p.get("ActiveZones") != Some("Battlefield") {
        return unmapped("key:Replacement.Draw.ActiveZones");
    }
    let word = p.get("ValidPlayer");
    let who = player_ref(word)?;
    let except_draw_step = u32::from(p.get("NotFirstCardInDrawStep") == Some("True"));
    let by = match (p.get("Prevent"), p.get("ReplaceWith")) {
        (Some("True"), None) => json!("Prevent"),
        (None, Some(name)) => {
            let part = ctx.sub(name)?;
            let read = ["DB", "Defined", "NumCards"];
            check_keys(&part, "Replacement.Draw", &read, |_, _| false)?;
            // "Draw two cards instead" is the player's own draw, doubled.
            let doubles = part.get("DB") == Some("Draw")
                && part.get("Defined") == Some("You")
                && part.get("NumCards") == Some("2")
                && word == Some("You");
            if !doubles {
                return unmapped("replacement:Draw-instead");
            }
            json!({"Multiply": 2})
        }
        _ => return unmapped("replacement:Draw-shape"),
    };
    let when = json!({"Draw": {"who": who, "except_draw_step": except_draw_step}});
    Ok(json!({"Replacement": {"when": when, "by": by}}))
}

/// `Event$ AddCounter`: counters put on permanents or on a player are doubled or increased.
pub fn add_counter(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let read = [
        "Event",
        "ValidCard",
        "ValidPlayer",
        "ValidCounterType",
        "ReplaceWith",
        "ActiveZones",
    ];
    check_keys(p, "Replacement.AddCounter", &read, |_, _| false)?;
    if p.get("ActiveZones") != Some("Battlefield") {
        return unmapped("key:Replacement.AddCounter.ActiveZones");
    }
    let on = match (p.get("ValidCard"), p.get("ValidPlayer")) {
        (Some(valid), None) => json!({"Each": selector_any(ctx, valid)?}),
        (None, Some(_)) => player_ref(p.get("ValidPlayer"))?,
        _ => return unmapped("replacement:AddCounter-shape"),
    };
    let Some(name) = p.get("ReplaceWith") else {
        return unmapped("replacement:AddCounter-without-ReplaceWith");
    };
    let part = ctx.sub(name)?;
    let read = ["DB", "ValidCounterType", "ChooseCounter", "Amount"];
    check_keys(&part, "Replacement.AddCounter", &read, |_, _| false)?;
    if part.get("DB") != Some("ReplaceCounter") {
        return unmapped("replacement:AddCounter-instead");
    }
    let by = part_change(ctx, part.get("Amount"), "CounterNum")?;
    let kind = match [p.get("ValidCounterType"), part.get("ValidCounterType")] {
        [Some(a), Some(b)] if a != b => return unmapped("key:Replacement.AddCounter.CounterType"),
        [Some(kind), _] | [None, Some(kind)] => Some(kind),
        [None, None] => None,
    };
    // Without a kind a run of several kinds is one event: only "twice" reads the same for each, and a chosen kind is
    // the one that changes.
    let chosen = part.get("ChooseCounter") == Some("True");
    if kind.is_none() && (by.get("Add").is_some() || chosen) {
        return unmapped("replacement:AddCounter-plus-without-a-kind");
    }
    let mut when = json!({ "on": on });
    if let Some(kind) = kind {
        let Some(kind) = counter_kind(kind) else {
            return unmapped("key:Replacement.AddCounter.CounterType");
        };
        when["counter"] = kind;
    }
    Ok(json!({"Replacement": {"when": {"CountersPut": when}, "by": by}}))
}
