//! Copies and retargeting: `Clone` (a permanent becomes a copy, or enters as one) and `ChangeTargets`.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, only, unmapped};
use super::keyword::keyword_list;
use super::valid::{defined, selector_any, spell_selector, subject};
use crate::forge::script::Params;

/// The effects of the copy verbs this module reads; `None` for any other API.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Option<Tr<Vec<Value>>> {
    match api {
        "Clone" => Some(clone(ctx, p)),
        "ChangeTargets" => Some(change_targets(ctx, p).map(|node| vec![node])),
        _ => None,
    }
}

/// The card types a word of `AddTypes$` may name; any other word is a subtype or a supertype.
const CARD_TYPES: [&str; 8] = [
    "Creature",
    "Artifact",
    "Enchantment",
    "Land",
    "Planeswalker",
    "Instant",
    "Sorcery",
    "Battle",
];

/// The exceptions of a copy (CR 707.9b) the keys of a `Clone` line stand for. `this_ability` says whether the line may
/// keep the ability it is part of (`GainThisAbility$`).
fn exceptions(ctx: &mut Ctx, p: &Params, this_ability: bool) -> Tr<Vec<Value>> {
    let mut mods = Vec::new();
    if let Some(name) = p.get("NewName") {
        mods.push(json!({ "SetName": name }));
    }
    if p.get("NonLegendary") == Some("True") {
        mods.push(json!({"RemoveSupertype": "Legendary"}));
    }
    for word in p.get("AddTypes").unwrap_or_default().split('&') {
        let word = word.trim();
        if word.is_empty() {
            continue;
        }
        mods.push(match word {
            "Legendary" | "Snow" | "Basic" => json!({ "AddSupertype": word }),
            _ if CARD_TYPES.contains(&word) => json!({ "AddType": word }),
            _ => json!({ "AddSubtype": word }),
        });
    }
    let loses_subtypes = p.has("RemoveSubTypes");
    if p.get("RemoveCardTypes") == Some("True") {
        // The card types the line adds are the only ones left, and a creature type goes with the creature type (CR 205.1a).
        let kept: Vec<Value> = mods
            .iter()
            .filter_map(|m| m.get("AddType").cloned())
            .collect();
        if kept.is_empty() || kept.contains(&json!("Creature")) {
            return unmapped("key:Clone.RemoveCardTypes");
        }
        mods.retain(|m| m.get("AddType").is_none());
        mods.push(json!({ "SetTypes": kept }));
    } else if loses_subtypes {
        return unmapped("key:Clone.RemoveSubTypes");
    }
    match (p.get("SetPower"), p.get("SetToughness")) {
        (None, None) => {}
        (Some(power), Some(toughness)) => {
            let (Ok(power), Ok(toughness)) = (power.parse::<i32>(), toughness.parse::<i32>())
            else {
                return unmapped("key:Clone.SetPower-count");
            };
            mods.push(json!({"SetPT": {"power": power, "toughness": toughness}}));
        }
        _ => return unmapped("key:Clone.SetPower-alone"),
    }
    if let Some(color) = p.get("SetColor") {
        if !matches!(color, "White" | "Blue" | "Black" | "Red" | "Green") {
            return unmapped("key:Clone.SetColor");
        }
        mods.push(json!({ "SetColors": [color] }));
    }
    if let Some(words) = p.get("AddKeywords") {
        for keyword in keyword_list_of(ctx, words)? {
            mods.push(json!({ "AddKeyword": keyword }));
        }
    }
    if p.get("GainThisAbility") == Some("True") {
        if !this_ability {
            return unmapped("key:Clone.GainThisAbility-replacement");
        }
        mods.push(json!("ThisAbility"));
    }
    mods.extend(granted_exceptions(ctx, p)?);
    Ok(mods)
}

/// `AddTriggers$`, `AddAbilities$` and `AddStaticAbilities$` of a copy: "except it has '<ability>'", each named part an
/// ability the copiable values gain (CR 707.9a), written in the order the card words them (triggered, activated, static).
/// `AddSVars$` only hands the copy the parts these abilities use, which the translation holds already.
pub(super) fn granted_exceptions(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    // Each ability is a line of its own: its targets are not those of the ability that makes the copy.
    ctx.nested(None, |ctx| granted_parts(ctx, p))
}

fn granted_parts(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    let mut mods = Vec::new();
    // The parts are the copy's in Forge only; the abilities below are translated with them inline.
    p.has("AddSVars");
    for (key, trigger) in [("AddTriggers", true), ("AddAbilities", false)] {
        if let Some(names) = p.get(key) {
            mods.extend(super::statics::granted(
                ctx,
                &names.replace(',', "&"),
                trigger,
            )?);
        }
    }
    if let Some(names) = p.get("AddStaticAbilities") {
        let before = std::mem::replace(&mut ctx.granted, true);
        let found = super::statics::granted_statics(ctx, &names.replace(',', "&"));
        ctx.granted = before;
        mods.extend(found?);
    }
    Ok(mods)
}

fn keyword_list_of(ctx: &Ctx, words: &str) -> Tr<Vec<Value>> {
    keyword_list(ctx, words)
}

/// The keys a `Clone` line may have, besides the exceptions.
const CLONE_KEYS: [&str; 26] = [
    "IntoPlayTapped",
    "RemoveCardTypes",
    "RemoveSubTypes",
    "AddTriggers",
    "AddAbilities",
    "AddStaticAbilities",
    "AddSVars",
    "ChoiceZone",
    "CloneTarget",
    "Defined",
    "ValidTgts",
    "Choices",
    "Duration",
    "Optional",
    "NewName",
    "NonLegendary",
    "AddTypes",
    "SetPower",
    "SetToughness",
    "SetColor",
    "AddKeywords",
    "GainThisAbility",
    "TargetMin",
    "TargetMax",
    "ChoiceTitle",
    "TgtPrompt",
];

/// `Clone`: the card itself becomes a copy of what `Defined$` or a target names, or (with `Choices$`) of a creature the
/// player picks, for the `Duration$` (for good without one). With `Choices$` and a target the target becomes the copy.
fn clone(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "Clone", &CLONE_KEYS)?;
    if p.has("ChoiceZone") {
        return unmapped("key:Clone.ChoiceZone");
    }
    if p.has("IntoPlayTapped") {
        return unmapped("key:Clone.IntoPlayTapped");
    }
    // "a copy of up to one target": with no target the effect copies nothing, which the engine reads as no change.
    let up_to_one = p.get("TargetMin") == Some("0")
        && p.get("TargetMax") == Some("1")
        && p.has("ValidTgts")
        && !p.has("Choices");
    if (p.has("TargetMin") || p.has("TargetMax")) && !up_to_one {
        return unmapped("key:Clone.TargetMin");
    }
    let duration = match p.get("Duration") {
        None => "Forever",
        Some("UntilEndOfTurn") => "UntilEndOfTurn",
        Some("UntilYourNextTurn") => "UntilYourNextTurn",
        Some(other) => return unmapped(format!("key:Clone.Duration={other}")),
    };
    // `CloneTarget$ Self` names what a clone line does without it: the card itself becomes the copy.
    // `ParentTarget` with a target of its own: the permanent the earlier step targeted becomes a copy of the new target.
    // `Valid <kind>` with `Defined$ Targeted`: every permanent of the kind (but the target, with `NotDefinedTargeted`)
    // becomes a copy of the target.
    let mut group: Option<(Vec<Value>, bool)> = None;
    let becomes_parent = match p.get("CloneTarget") {
        None | Some("Self") => false,
        Some("ParentTarget") if p.has("ValidTgts") && !p.has("Choices") => true,
        Some(valid)
            if valid.starts_with("Valid ")
                && p.get("Defined") == Some("Targeted")
                && !p.has("Choices") =>
        {
            let kind = valid.trim_start_matches("Valid ");
            let (kind, but_target) = match kind
                .strip_suffix(".NotDefinedTargeted")
                .or_else(|| kind.strip_suffix("+NotDefinedTargeted"))
            {
                Some(rest) => (rest, true),
                None => (kind, false),
            };
            group = Some((selector_any(ctx, kind)?, but_target));
            false
        }
        Some(other) => return unmapped(format!("key:Clone.CloneTarget={other}")),
    };
    if p.has("ValidTgts") && p.has("Defined") && group.is_none() {
        return unmapped("key:Clone.ValidTgts-and-Defined");
    }
    // The creature whose entering triggered the ability is still on the battlefield as the trigger resolves, or gone; a
    // card that moved to another zone would be copied as the card there, not as the creature it was.
    let entered = ctx.trigger.as_ref().is_some_and(|shape| {
        shape.mode == "ChangesZone"
            && !shape.itself
            && shape.destination.as_deref() == Some("Battlefield")
    });
    if let Some(word) = p.get("Defined")
        && !matches!(word, "Targeted" | "ParentTarget")
        && !(entered && matches!(word, "TriggeredCard" | "TriggeredCardLKICopy"))
    {
        return unmapped(format!("defined:Clone.{word}"));
    }
    let except = exceptions(ctx, p, true)?;
    let mut effects = Vec::new();
    let (what, of) = match (p.get("Choices"), p.has("ValidTgts"), p.get("Defined")) {
        (Some(choices), has_target, None) => {
            let name = ctx.fresh("copied");
            let from = json!({ "Each": selector_any(ctx, choices)? });
            effects.push(json!({"Choose": {"from": from, "as": name}}));
            let what = if has_target {
                subject(ctx, p, "", None)?
            } else {
                json!("This")
            };
            (what, json!({ "Bound": name }))
        }
        (None, _, Some(_)) if group.is_some() => {
            group_copies(ctx, p, group.take().unwrap_or_default())?
        }
        (None, _, _) if becomes_parent => parent_becomes_copy(ctx, p)?,
        (None, _, _) => (json!("This"), subject(ctx, p, "", None)?),
        (Some(_), _, Some(_)) => return unmapped("key:Clone.Choices-and-Defined"),
    };
    let copy_node =
        json!({"BecomeCopy": {"what": what, "of": of, "except": except, "duration": duration}});
    if p.get("Optional") == Some("True") {
        effects.push(json!({"May": {"effects": [copy_node]}}));
    } else {
        effects.push(copy_node);
    }
    Ok(effects)
}

/// `Valid <kind>` with `Defined$ Targeted`: the permanents of the kind (but the target, with `but_target`) that become
/// copies, and the target they copy.
fn group_copies(ctx: &mut Ctx, p: &Params, group: (Vec<Value>, bool)) -> Tr<(Value, Value)> {
    let (mut kind, but_target) = group;
    let target = if p.has("ValidTgts") {
        subject(ctx, &p.without(&["Defined"]), "", None)?
    } else {
        subject(ctx, p, "", None)?
    };
    if but_target {
        kind.push(json!({"Not": {"Is": target.clone()}}));
    }
    Ok((json!({ "Each": kind }), target))
}

/// `CloneTarget$ ParentTarget`: the object the earlier step targeted, and the new target it copies.
fn parent_becomes_copy(ctx: &mut Ctx, p: &Params) -> Tr<(Value, Value)> {
    let Ok(becomes) = ctx.parent() else {
        return unmapped("key:Clone.CloneTarget=ParentTarget");
    };
    let target = subject(ctx, p, "", None)?;
    // "Another target" is a second target that is not the first, which the line states with `TargetUnique$`.
    let another = p
        .get("TgtPrompt")
        .is_some_and(|text| text.contains("another"));
    let unique = p
        .get("ValidTgts")
        .is_some_and(|valid| valid.contains("NotTarget"));
    if another && !unique {
        return unmapped("key:Clone.another-target");
    }
    Ok((becomes, target))
}

/// `K:ETBReplacement:Copy:<part>:Optional`: the permanent may enter as a copy of a permanent `Choices$` names.
pub fn enters_as_copy(ctx: &mut Ctx, name: &str) -> Tr<Value> {
    let Some(text) = ctx.svar(name).map(str::to_owned) else {
        return unmapped("svar:missing");
    };
    let p = Params::parse(&text);
    if p.get("DB") != Some("Clone") {
        return unmapped("keyword:ETBReplacement-Copy-part");
    }
    only(&p, "Clone", &CLONE_KEYS)?;
    if p.has("CloneTarget") {
        return unmapped("key:Clone.replacement-CloneTarget");
    }
    if p.has("Defined") || p.has("ValidTgts") || p.has("Optional") {
        return unmapped("key:Clone.replacement-shape");
    }
    // "As it enters, you may have it become a copy ... until end of turn" (Cursed Mirror).
    let until_end_of_turn = match p.get("Duration") {
        None => false,
        Some("UntilEndOfTurn") => true,
        Some(_) => return unmapped("key:Clone.replacement-shape"),
    };
    // The replacement is the copy alone: a later step (Spark Double's extra counter) would be dropped.
    if p.has("SubAbility") {
        return unmapped("key:Clone.replacement-SubAbility");
    }
    let Some(choices) = p.get("Choices") else {
        return unmapped("key:Clone.no-Choices");
    };
    let of = selector_any(ctx, choices)?;
    let mut of = super::fix26_printed::copy_of(of, p.get("SpellDescription"));
    match p.get("ChoiceZone") {
        None | Some("Battlefield") => {}
        Some("Graveyard") => of.push(json!({"In": "Graveyard"})),
        Some(other) => return unmapped(format!("key:Clone.ChoiceZone={other}")),
    }
    let mut by = json!({"AsCopy": {"of": of}});
    if p.get("IntoPlayTapped") == Some("True") {
        by["AsCopy"]["tapped"] = json!(true);
    }
    if until_end_of_turn {
        by["AsCopy"]["until_end_of_turn"] = json!(true);
    }
    let except = exceptions(ctx, &p, false)?;
    if !except.is_empty() {
        by["AsCopy"]["except"] = Value::Array(except);
    }
    Ok(by)
}

/// `ChangeTargets`: the targets of a spell or ability on the stack change; "change the target" (a single target, another
/// one) or "you may choose new targets".
fn change_targets(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "ChangeTargets",
        &["TargetType", "ValidTgts", "Optional", "Defined"],
    )?;
    let to = if p.get("Optional") == Some("True") {
        "Choose"
    } else {
        "Another"
    };
    if p.has("Defined") {
        if to != "Choose" || p.has("ValidTgts") {
            return unmapped("key:ChangeTargets.Defined-shape");
        }
        let what = defined(ctx, p.get("Defined").unwrap_or_default())?;
        return Ok(json!({"ChangeTargets": {"what": what, "to": to}}));
    }
    let Some(kind) = p.get("TargetType") else {
        return unmapped("key:ChangeTargets.no-TargetType");
    };
    let (kind, single) = match kind.strip_suffix(".singleTarget") {
        Some(kind) => (kind, true),
        None => (kind, false),
    };
    if to == "Another" && !single {
        return unmapped("key:ChangeTargets.Another-without-single-target");
    }
    let mut selector = vec![match kind {
        "Spell" => json!("Spell"),
        "SpellAbility" | "Spell,Activated,Triggered" => {
            json!({"AnyOf": ["Spell", {"Ability": "Activated"}, {"Ability": "Triggered"}]})
        }
        other => return unmapped(format!("key:ChangeTargets.TargetType={other}")),
    }];
    if single {
        selector.push(json!("SingleTarget"));
    }
    let valid: Vec<&str> = p
        .get("ValidTgts")
        .unwrap_or("Card")
        .split(',')
        .filter(|word| *word != "Emblem")
        .collect();
    if valid != ["Card"] {
        selector.extend(spell_selector(ctx, &valid.join(","))?);
    }
    let what = ctx.target(json!({"what": {"Object": selector}}))?;
    Ok(json!({"ChangeTargets": {"what": what, "to": to}}))
}
