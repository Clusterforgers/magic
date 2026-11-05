//! Copies and retargeting: `Clone` (a permanent becomes a copy, or enters as one) and `ChangeTargets`.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, only, unmapped};
use super::keyword::keyword_list;
use super::valid::{defined, selector_any, subject};
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
fn exceptions(ctx: &Ctx, p: &Params, this_ability: bool) -> Tr<Vec<Value>> {
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
    Ok(mods)
}

fn keyword_list_of(ctx: &Ctx, words: &str) -> Tr<Vec<Value>> {
    keyword_list(ctx, words)
}

/// The keys a `Clone` line may have, besides the exceptions.
const CLONE_KEYS: [&str; 19] = [
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
    if p.has("TargetMin") || p.has("TargetMax") {
        return unmapped("key:Clone.TargetMin");
    }
    let duration = match p.get("Duration") {
        None => "Forever",
        Some("UntilEndOfTurn") => "UntilEndOfTurn",
        Some("UntilYourNextTurn") => "UntilYourNextTurn",
        Some(other) => return unmapped(format!("key:Clone.Duration={other}")),
    };
    // `CloneTarget$ Self` names what a clone line does without it: the card itself becomes the copy.
    match p.get("CloneTarget") {
        None | Some("Self") => {}
        Some(other) => return unmapped(format!("key:Clone.CloneTarget={other}")),
    }
    if p.has("ValidTgts") && p.has("Defined") {
        return unmapped("key:Clone.ValidTgts-and-Defined");
    }
    if let Some(word) = p.get("Defined")
        && !matches!(word, "Targeted" | "ParentTarget")
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

/// `K:ETBReplacement:Copy:<part>:Optional`: the permanent may enter as a copy of a permanent `Choices$` names.
pub fn enters_as_copy(ctx: &Ctx, name: &str) -> Tr<Value> {
    let Some(text) = ctx.svar(name) else {
        return unmapped("svar:missing");
    };
    let p = Params::parse(text);
    if p.get("DB") != Some("Clone") {
        return unmapped("keyword:ETBReplacement-Copy-part");
    }
    only(&p, "Clone", &CLONE_KEYS)?;
    if p.has("CloneTarget") {
        return unmapped("key:Clone.replacement-CloneTarget");
    }
    if p.has("Defined") || p.has("ValidTgts") || p.has("Duration") || p.has("Optional") {
        return unmapped("key:Clone.replacement-shape");
    }
    let Some(choices) = p.get("Choices") else {
        return unmapped("key:Clone.no-Choices");
    };
    let mut of = selector_any(ctx, choices)?;
    match p.get("ChoiceZone") {
        None | Some("Battlefield") => {}
        Some("Graveyard") => of.push(json!({"In": "Graveyard"})),
        Some(other) => return unmapped(format!("key:Clone.ChoiceZone={other}")),
    }
    let mut by = json!({"AsCopy": {"of": of}});
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
        selector.extend(selector_any(ctx, &valid.join(","))?);
    }
    let what = ctx.target(json!({"what": {"Object": selector}}))?;
    Ok(json!({"ChangeTargets": {"what": what, "to": to}}))
}
