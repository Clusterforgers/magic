//! `Effect`: Forge's pseudo-object that holds a static ability for a while ("target creature can't be blocked this turn")
//! as a continuous effect or rule of the DSL.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, check_keys, only, unmapped};
use super::effects_obj::duration;
use super::keyword::keyword_list;
use super::valid::{defined, objects, selector_any, subject};
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
            "Boon",
            "ForgetCounter",
            "ForgetOnCast",
            "EffectOwner",
        ],
    )?;
    if p.has("ForgetOnCast") {
        return super::effects_next_spell::effects(ctx, p);
    }
    // The Forge effect object ends when what it remembers changes zones; our references to remembered objects drop out the
    // same way, so the keys only matter beside `RememberObjects`.
    // Who holds the effect: only a permission to cast reads it, as the player who may cast.
    let owner = match p.get("EffectOwner") {
        None | Some("You") => None,
        Some(word) => Some(defined(ctx, word)?),
    };
    let remembering = p.has("RememberObjects") || p.has("ValidTgts");
    if owner.is_some()
        && (p.has("Triggers")
            || p.has("ReplacementEffects")
            || !remembering
            || super::effects_emblem::is_emblem(p))
    {
        return unmapped("key:Effect.EffectOwner");
    }
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
        let mut out = ambient_with(ctx, p, remembered.as_ref())?;
        out.extend(super::effects_ambient::delayed_triggers(
            ctx,
            p,
            remembered.as_ref(),
        )?);
        return Ok(out);
    }
    if let Some(name) = p
        .get("ReplacementEffects")
        .filter(|name| !name.contains(','))
        && ctx.sub(name.trim())?.get("Event") == Some("Untap")
    {
        return super::replacements::stays_tapped(ctx, p);
    }
    if let Some(name) = p
        .get("ReplacementEffects")
        .filter(|name| !name.contains(','))
        .filter(|_| !p.has("RememberObjects") && !p.has("ValidTgts") && !p.has("StaticAbilities"))
        .map(str::trim)
        .filter(|name| {
            ctx.sub(name)
                .is_ok_and(|sub| sub.get("Event") == Some("ProduceMana"))
        })
    {
        return super::replacements_mana::effect(ctx, p, name);
    }
    if p.has("ReplacementEffects") {
        let remembered = super::replacements_damage::remembered_of(ctx, p)?;
        let mut out = ambient_with(ctx, p, remembered.as_ref())?;
        out.extend(super::replacements_damage::shield(
            ctx,
            p,
            remembered.as_ref(),
        )?);
        return Ok(out);
    }
    if !p.has("RememberObjects") && !p.has("ValidTgts") {
        return super::effects_ambient::continuous_statics(ctx, p);
    }
    remembered_statics(ctx, p, owner.as_ref(), None)
}

/// The statics of an `Effect` line about the objects it remembers.
fn remembered_statics(
    ctx: &mut Ctx,
    p: &Params,
    owner: Option<&Value>,
    known: Option<&Value>,
) -> Tr<Vec<Value>> {
    let Some(names) = p.get("StaticAbilities") else {
        return unmapped("key:Effect.no-StaticAbilities");
    };
    let duration = lasting(p)?;
    let who = match known {
        Some(who) => who.clone(),
        None => remembered(ctx, p)?,
    };
    let mut out = Vec::new();
    for name in names.split(',').map(str::trim) {
        out.extend(remembered_static(ctx, name, &who, &duration, owner)?);
    }
    Ok(out)
}

/// The statics of an `Effect` line that has no remembered objects, when the line also holds a trigger or a replacement.
fn ambient_with(ctx: &mut Ctx, p: &Params, known: Option<&Value>) -> Tr<Vec<Value>> {
    if !p.has("StaticAbilities") {
        return Ok(Vec::new());
    }
    if known.is_some() {
        return remembered_statics(ctx, p, None, known);
    }
    if p.has("RememberObjects") || p.has("ValidTgts") {
        return unmapped("key:Effect.static-with-remembered-and-more");
    }
    super::effects_ambient::continuous_statics(ctx, p)
}

/// How long the objects stay remembered: the `Duration$`, or for as long as every one of them has a counter of the kind
/// `ForgetCounter$` names (an object that leaves the battlefield is a new one, CR 400.7, so it drops out the same way).
pub(super) fn lasting(p: &Params) -> Tr<Value> {
    let Some(kind) = p.get("ForgetCounter") else {
        return duration(p.get("Duration")).map(|word| json!(word));
    };
    if !p.has("RememberObjects")
        || p.get("Duration") != Some("Permanent")
        || p.get("ForgetOnMoved") != Some("Battlefield")
    {
        return unmapped("key:Effect.ForgetCounter-beyond-a-permanent-effect-on-the-battlefield");
    }
    match super::amount::counter_kind(kind) {
        Some(kind) => Ok(json!({"WhileHasCounter": kind})),
        None => unmapped(format!("key:Effect.ForgetCounter={kind}")),
    }
}

/// One static of an `Effect` line about the objects it remembers.
fn remembered_static(
    ctx: &mut Ctx,
    name: &str,
    who: &Value,
    duration: &Value,
    owner: Option<&Value>,
) -> Tr<Vec<Value>> {
    let part = ctx.sub(name)?;
    if owner.is_some() && !part.has("MayPlay") {
        return unmapped("key:Effect.EffectOwner");
    }
    // A prohibition on the remembered players ("target player cannot cast spells this turn").
    if let Some(found) = super::statics_cant::rule_for(ctx, &part, Some(who)) {
        let mut rule = found?;
        rule["Rule"]["duration"] = duration.clone();
        return Ok(vec![rule]);
    }
    let who = &objects(who.clone())?;
    if part.has("MayPlay") {
        let mut rule = super::mayplay::may_cast(ctx, &part, Some(who))?;
        if let Some(owner) = owner {
            rule["MayCast"]["who"] = owner.clone();
        }
        return Ok(vec![json!({"Rule": {"rule": rule, "duration": duration}})]);
    }
    let read = [
        "Mode",
        "ValidAttacker",
        "ValidBlocker",
        "ValidCreature",
        "ValidCard",
        "Affected",
        "AddKeyword",
        "AddType",
        "AddPower",
        "AddToughness",
    ];
    // `ValidTarget$` and `Activator$` belong to a `CantTarget` line alone.
    let cant_target = part.get("Mode") == Some("CantTarget");
    let read: Vec<&str> = if cant_target {
        [read.as_slice(), &["ValidTarget", "Activator"]].concat()
    } else {
        read.to_vec()
    };
    check_keys(&part, "Effect.static", &read, |_, _| false)?;
    let on_remembered = |key: &str| {
        matches!(
            part.get(key),
            Some("Card.IsRemembered" | "Creature.IsRemembered")
        )
    };
    match part.get("Mode").unwrap_or_default() {
        "CantBlockBy" if on_remembered("ValidAttacker") && !part.has("ValidBlocker") => {
            Ok(vec![continuous(who, &[json!("CantBeBlocked")], duration)])
        }
        "CantBlockBy" if on_remembered("ValidAttacker") => {
            let blockers = selector_any(ctx, part.get("ValidBlocker").unwrap_or_default())?;
            let by = json!({"CantBeBlockedBy": blockers});
            Ok(vec![continuous(who, &[by], duration)])
        }
        // "Can't block EFFECTSOURCE": the remembered creature blocks only what is not the source.
        "CantBlockBy"
            if part.get("ValidAttacker") == Some("Creature.EffectSource")
                && on_remembered("ValidBlocker") =>
        {
            let only = json!({"CanBlockOnly": [{"Not": {"Is": "This"}}]});
            Ok(vec![continuous(who, &[only], duration)])
        }
        "AssignNoCombatDamage" if on_remembered("ValidCard") => Ok(vec![json!({"Rule": {
            "rule": {"Combat": {"who": who, "quirk": "AssignsNoCombatDamage"}}, "duration": duration}})]),
        "CantRegenerate" if on_remembered("ValidCard") => Ok(vec![
            json!({"Rule": {"rule": {"CantBeRegenerated": who}, "duration": duration}}),
        ]),
        "MustBlock" if on_remembered("ValidCreature") => Ok(vec![
            json!({"Rule": {"rule": {"MustBlock": who}, "duration": duration}}),
        ]),
        "CantAttack" if on_remembered("ValidCard") => {
            Ok(vec![continuous(who, &[json!("CantAttack")], duration)])
        }
        "CantBlock" if on_remembered("ValidCard") => {
            Ok(vec![continuous(who, &[json!("CantBlock")], duration)])
        }
        // What the opponents of the effect's controller can't target is what hexproof says (CR 702.11b).
        "CantTarget"
            if on_remembered("ValidTarget") && part.get("Activator") == Some("Player.Opponent") =>
        {
            Ok(vec![continuous(
                who,
                &[json!({"AddKeyword": "Hexproof"})],
                duration,
            )])
        }
        "MustAttack" if on_remembered("ValidCreature") => Ok(vec![
            json!({"Rule": {"rule": {"MustAttack": who}, "duration": duration}}),
        ]),
        "Continuous" if on_remembered("Affected") => {
            let mods = pump_like(ctx, &part)?;
            Ok(vec![continuous(who, &mods, duration)])
        }
        other => unmapped(format!("effect:Effect-static-{other}")),
    }
}

fn continuous(affected: &Value, mods: &[Value], duration: &Value) -> Value {
    json!({"Continuous": {"affected": affected, "mods": mods, "duration": duration}})
}

/// The objects an effect is about: `RememberObjects$` names them (the target, the source, or what an earlier step
/// moved).
pub(super) fn remembered(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let word = p.get("RememberObjects").unwrap_or_default();
    // `Defined$` names the same objects the effect remembers.
    if let Some(defined) = p
        .get("Defined")
        .filter(|defined| Some(*defined) != p.get("RememberObjects"))
    {
        return unmapped(format!(
            "key:Effect.Defined={defined}-beside-RememberObjects"
        ));
    }
    let who = match word {
        "Targeted" | "ThisTargetedCard" if p.has("ValidTgts") => subject(ctx, p, "", None)?,
        "Self" => json!("This"),
        "Targeted" => ctx.last_target()?,
        "Remembered" | "RememberedCard" => defined(ctx, "Remembered")?,
        // A word another step of the line already reads as an object: the choice, the imprint, the enchanted creature.
        "ChosenCard" | "Imprinted" | "Enchanted" => defined(ctx, word)?,
        "TriggeredCard" => super::triggered::resolve(ctx.trigger.as_ref(), "TriggeredCard")?,
        word if super::triggered::is_word(word) => {
            super::triggered::remembered_by_event(ctx.trigger.as_ref(), word)?
        }
        "ParentTarget" => ctx.parent()?,
        other => return unmapped(format!("key:Effect.RememberObjects={other}")),
    };
    Ok(who)
}

/// Power, toughness and keywords of a continuous static line.
fn pump_like(ctx: &Ctx, part: &Params) -> Tr<Vec<Value>> {
    let mut mods = Vec::new();
    if part.has("AddPower") || part.has("AddToughness") {
        let power = amount(ctx, part.get("AddPower").unwrap_or("0"))?;
        let toughness = amount(ctx, part.get("AddToughness").unwrap_or("0"))?;
        mods.push(json!({"ModifyPT": {"power": power, "toughness": toughness}}));
    }
    mods.extend(super::effects_zone::added_types(
        part.get("AddType").unwrap_or_default(),
    ));
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
