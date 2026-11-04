//! `Effect`: Forge's pseudo-object that holds a static ability for a while ("target creature can't be blocked this turn")
//! as a continuous effect or rule of the DSL.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, check_keys, only, unmapped};
use super::effects_obj::duration;
use super::keyword::keyword_list;
use super::valid::{defined, objects, subject};
use crate::forge::script::Params;

/// The effects of an `Effect` line.
pub fn effects(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Effect",
        &[
            "RememberObjects",
            "StaticAbilities",
            "ExileOnMoved",
            "ForgetOnMoved",
            "Duration",
            "ValidTgts",
            "TargetMin",
            "TargetMax",
            "Name",
            "Stackable",
            "Defined",
            "ReplacementEffects",
            "Triggers",
        ],
    )?;
    // The Forge effect object ends when what it remembers changes zones; our references to remembered objects drop out the
    // same way, so the keys only matter beside `RememberObjects`.
    if (p.has("ExileOnMoved") || p.has("ForgetOnMoved")) && !p.has("RememberObjects") {
        return unmapped("key:Effect.ExileOnMoved-without-RememberObjects");
    }
    if super::effects_emblem::is_emblem(p) {
        return super::effects_emblem::emblem(ctx, p);
    }
    if p.has("Triggers") {
        if p.has("ReplacementEffects") {
            return unmapped("key:Effect.Triggers-and-ReplacementEffects");
        }
        let remembered = super::replacements_damage::remembered_of(ctx, p)?;
        return super::effects_ambient::delayed_triggers(ctx, p, remembered.as_ref());
    }
    if p.has("ReplacementEffects") {
        if p.has("StaticAbilities") {
            return unmapped("key:Effect.static-and-replacement");
        }
        let remembered = super::replacements_damage::remembered_of(ctx, p)?;
        return super::replacements_damage::shield(ctx, p, remembered.as_ref());
    }
    if !p.has("RememberObjects") && !p.has("ValidTgts") {
        return super::effects_ambient::continuous_statics(ctx, p);
    }
    let Some(name) = p.get("StaticAbilities") else {
        return unmapped("key:Effect.no-StaticAbilities");
    };
    if name.contains(',') {
        return unmapped("key:Effect.StaticAbilities-list");
    }
    let duration = duration(p.get("Duration"))?;
    let who = remembered(ctx, p)?;
    let part = ctx.sub(name)?;
    if part.has("MayPlay") {
        let rule = super::mayplay::may_cast(ctx, &part, Some(&who))?;
        return Ok(vec![json!({"Rule": {"rule": rule, "duration": duration}})]);
    }
    let read = [
            "Mode",
            "ValidAttacker",
            "ValidCreature",
            "ValidCard",
            "Affected",
            "AddKeyword",
            "AddPower",
            "AddToughness",
    ];
    check_keys(&part, "Effect.static", &read, |_, _| false)?;
    let on_remembered = |key: &str| {
        matches!(
            part.get(key),
            Some("Card.IsRemembered" | "Creature.IsRemembered")
        )
    };
    match part.get("Mode").unwrap_or_default() {
        "CantBlockBy" if on_remembered("ValidAttacker") && !part.has("ValidBlocker") => {
            Ok(vec![continuous(&who, &[json!("CantBeBlocked")], duration)])
        }
        "CantAttack" if on_remembered("ValidCard") => {
            Ok(vec![continuous(&who, &[json!("CantAttack")], duration)])
        }
        "CantBlock" if on_remembered("ValidCard") => {
            Ok(vec![continuous(&who, &[json!("CantBlock")], duration)])
        }
        "MustAttack" if on_remembered("ValidCreature") => Ok(vec![
            json!({"Rule": {"rule": {"MustAttack": who}, "duration": duration}}),
        ]),
        "Continuous" if on_remembered("Affected") => {
            let mods = pump_like(ctx, &part)?;
            Ok(vec![continuous(&who, &mods, duration)])
        }
        other => unmapped(format!("effect:Effect-static-{other}")),
    }
}

fn continuous(affected: &Value, mods: &[Value], duration: &str) -> Value {
    json!({"Continuous": {"affected": affected, "mods": mods, "duration": duration}})
}

/// The objects an effect is about: `RememberObjects$` names them (the target, the source, or what an earlier step
/// moved).
fn remembered(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let word = p.get("RememberObjects").unwrap_or_default();
    let who = match word {
        "Targeted" if p.has("ValidTgts") => subject(ctx, p, "", None)?,
        "Self" => json!("This"),
        "Targeted" => ctx.last_target()?,
        "Remembered" | "RememberedCard" => defined(ctx, "Remembered")?,
        "TriggeredCard" => super::triggered::resolve(ctx.trigger.as_ref(), "TriggeredCard")?,
        "ParentTarget" => ctx.parent()?,
        other => return unmapped(format!("key:Effect.RememberObjects={other}")),
    };
    objects(who)
}

/// Power, toughness and keywords of a continuous static line.
fn pump_like(ctx: &Ctx, part: &Params) -> Tr<Vec<Value>> {
    let mut mods = Vec::new();
    if part.has("AddPower") || part.has("AddToughness") {
        let power = amount(ctx, part.get("AddPower").unwrap_or("0"))?;
        let toughness = amount(ctx, part.get("AddToughness").unwrap_or("0"))?;
        mods.push(json!({"ModifyPT": {"power": power, "toughness": toughness}}));
    }
    if let Some(keywords) = part.get("AddKeyword") {
        for keyword in keyword_list(ctx, keywords)? {
            mods.push(json!({ "AddKeyword": keyword }));
        }
    }
    if mods.is_empty() {
        return unmapped("effect:Effect-static-nothing");
    }
    Ok(mods)
}
