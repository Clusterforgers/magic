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
    // "Spell mastery — if there are two or more instant and/or sorcery cards in your graveyard" (`IsPresent$`) is the condition.
    let counted = p.has("IsPresent");
    let held = if counted {
        super::condition::condition(ctx, p)?
    } else {
        None
    };
    let stripped = counted.then(|| super::condition::strip(p));
    let p = stripped.as_ref().unwrap_or(p);
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
        let mut rule = json!({"Rule": {"rule": {"CantBeCountered": "This"}, "from": "Stack"}});
        if let Some(held) = held {
            rule["Rule"]["condition"] = held;
        }
        return Ok(rule);
    }
    if zone != Some("Battlefield") {
        return unmapped("replacement:Counter-zone");
    }
    if held.is_some() {
        return unmapped("key:Replacement.Counter.condition");
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
        "HalfDown" if what == "CounterNum" => Ok(json!("Half")),
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
    let read = [
        "Event",
        "ValidPlayer",
        "ReplaceWith",
        "ActiveZones",
        "Prevent",
    ];
    check_keys(p, "Replacement.GainLife", &read, |_, _| false)?;
    if p.get("ActiveZones") != Some("Battlefield") {
        return unmapped("key:Replacement.GainLife.ActiveZones");
    }
    let who = player_ref(p.get("ValidPlayer"))?;
    if p.get("Prevent") == Some("True") && !p.has("ReplaceWith") {
        return Ok(json!({"Replacement": {"when": {"LifeGain": {"who": who}}, "by": "Prevent"}}));
    }
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

/// `Event$ Mill`: a player mills twice that many cards, or that many plus a number.
pub fn mill(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let read = ["Event", "ValidPlayer", "ReplaceWith", "ActiveZones"];
    check_keys(p, "Replacement.Mill", &read, |_, _| false)?;
    if p.get("ActiveZones") != Some("Battlefield") {
        return unmapped("key:Replacement.Mill.ActiveZones");
    }
    let who = player_ref(p.get("ValidPlayer"))?;
    let Some(name) = p.get("ReplaceWith") else {
        return unmapped("replacement:Mill-without-ReplaceWith");
    };
    let part = ctx.sub(name)?;
    let read = ["DB", "VarName", "VarValue"];
    check_keys(&part, "Replacement.Mill", &read, |_, _| false)?;
    if part.get("DB") != Some("ReplaceEffect") || part.get("VarName") != Some("Number") {
        return unmapped("replacement:Mill-instead");
    }
    let by = part_change(ctx, part.get("VarValue"), "Number")?;
    Ok(json!({"Replacement": {"when": {"Mill": {"who": who}}, "by": by}}))
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
        "ValidSource",
        "ValidObject",
        "EffectOnly",
    ];
    check_keys(p, "Replacement.AddCounter", &read, |_, _| false)?;
    if p.get("ActiveZones") != Some("Battlefield") {
        return unmapped("key:Replacement.AddCounter.ActiveZones");
    }
    // "If you would put", "if an opponent would put": who puts the counters.
    let putter = match p.get("ValidSource") {
        None => None,
        Some("You") => Some("You"),
        Some("Opponent") => Some("Opponents"),
        Some(_) => return unmapped("key:Replacement.AddCounter.ValidSource"),
    };
    let effect = match p.get("EffectOnly") {
        None => false,
        Some("True") => true,
        Some(_) => return unmapped("key:Replacement.AddCounter.EffectOnly"),
    };
    let on = match (
        p.get("ValidCard"),
        p.get("ValidPlayer"),
        p.get("ValidObject"),
    ) {
        (Some(valid), None, None) => Some(json!({"Each": selector_any(ctx, valid)?})),
        (None, Some(_), None) => Some(player_ref(p.get("ValidPlayer"))?),
        (None, None, Some("Permanent.inZoneBattlefield,Player" | "Permanent,Player")) => None,
        (None, None, Some(valid)) if !valid.contains(',') => {
            Some(json!({"Each": selector_any(ctx, valid.trim_end_matches("+inZoneBattlefield"))?}))
        }
        _ => return unmapped("replacement:AddCounter-shape"),
    };
    let Some(name) = p.get("ReplaceWith") else {
        return unmapped("replacement:AddCounter-without-ReplaceWith");
    };
    let part = ctx.sub(name)?;
    let read = [
        "DB",
        "ValidCounterType",
        "ChooseCounter",
        "Amount",
        "ValidSource",
    ];
    check_keys(&part, "Replacement.AddCounter", &read, |_, _| false)?;
    if part.has("ValidSource") && part.get("ValidSource") != p.get("ValidSource") {
        return unmapped("key:Replacement.AddCounter.ValidSource");
    }
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
    let mut when = json!({});
    if let Some(on) = on {
        when["on"] = on;
    }
    if let Some(putter) = putter {
        when["by"] = json!(putter);
    }
    if effect {
        when["effect"] = json!(true);
    }
    if let Some(kind) = kind {
        let Some(kind) = counter_kind(kind) else {
            return unmapped("key:Replacement.AddCounter.CounterType");
        };
        when["counter"] = kind;
    }
    Ok(json!({"Replacement": {"when": {"CountersPut": when}, "by": by}}))
}

/// An `Event$ Counter` replacement named by an `Effect` line (`ReplacementEffects$`): "spells you control can't be
/// countered this turn" is a rule of the effect's duration over the spells the line names.
pub fn shield(ctx: &mut Ctx, p: &Params, duration: &str) -> Tr<Value> {
    check_keys(
        p,
        "Replacement.Counter.shield",
        &["Event", "Layer", "ValidSA"],
        |_, _| false,
    )?;
    if p.get("Layer") != Some("CantHappen") {
        return unmapped("replacement:Counter-instead");
    }
    let sa = p.get("ValidSA").unwrap_or_default();
    if sa.is_empty()
        || sa
            .split(',')
            .any(|one| one.split('.').next() != Some("Spell"))
    {
        return unmapped("replacement:Counter-abilities");
    }
    let preds = selector_any(ctx, sa)?;
    Ok(json!({"Rule": {"rule": {"CantBeCountered": {"Each": preds}}, "duration": duration}}))
}
