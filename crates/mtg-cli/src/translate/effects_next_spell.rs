//! `Effect` with `ForgetOnCast$`: a static line that applies to the next spell its controller casts and ends with that
//! cast (`GameRule::NextSpell`): a cost change, flash, a free cast, or a help keyword.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::effects_obj::duration;
use super::keyword::keyword_value;
use super::valid::selector_any;
use crate::forge::script::Params;

/// The effects of an `Effect` line that holds `ForgetOnCast$`.
pub fn effects(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    if p.has("RememberObjects") || p.has("ValidTgts") || p.has("Triggers") {
        return unmapped("key:Effect.ForgetOnCast-beyond-statics");
    }
    let Some(names) = p
        .get("StaticAbilities")
        .filter(|names| !names.contains(','))
    else {
        return unmapped("key:Effect.ForgetOnCast-StaticAbilities");
    };
    let spells = spells_of(p.get("ForgetOnCast").unwrap_or_default())?;
    let duration = duration(p.get("Duration"))?;
    if !matches!(duration, "UntilEndOfTurn" | "Forever") {
        return unmapped(format!("key:Effect.ForgetOnCast-Duration={duration}"));
    }
    let part = ctx.sub(names.trim())?;
    let benefit = benefit(ctx, &part, &spells)?;
    let what = selector_any(ctx, &spells.without_you)?;
    let mut rule = json!({ "benefit": benefit });
    if !what.is_empty() {
        rule["what"] = json!(what);
    }
    Ok(vec![
        json!({"Rule": {"rule": {"NextSpell": rule}, "duration": duration}}),
    ])
}

/// The spells a line is about: the valid text with the caster's `YouCtrl` taken out (the rule's controller is the caster).
struct Spells {
    without_you: String,
}

/// `Instant.YouCtrl,Sorcery.YouCtrl` as the spells of any alternative that the controller casts.
fn spells_of(valid: &str) -> Tr<Spells> {
    if valid.is_empty() || valid == "False" || valid.contains("IsRemembered") {
        return unmapped(format!("key:Effect.ForgetOnCast={valid}"));
    }
    let mut kept = Vec::new();
    for alternative in valid.split(',') {
        if !alternative.contains(".YouCtrl") {
            return unmapped("key:Effect.ForgetOnCast-not-your-spells");
        }
        kept.push(alternative.replacen(".YouCtrl", "", 1));
    }
    Ok(Spells {
        without_you: kept.join(","),
    })
}

/// What the static line gives the spell.
fn benefit(ctx: &Ctx, part: &Params, spells: &Spells) -> Tr<Value> {
    // The line may repeat which spells it covers, in the words of the lines that name the spell: it must be the same set.
    let same = |key: &str| match part.get(key) {
        None => true,
        Some(text) => text.replace(".YouCtrl", "") == spells.without_you,
    };
    match part.get("Mode").unwrap_or_default() {
        mode @ ("ReduceCost" | "RaiseCost") => {
            check_keys(
                part,
                "Effect.ForgetOnCast.cost",
                &["Mode", "Type", "Activator", "Amount", "ValidCard"],
                |_, _| false,
            )?;
            if part.get("Type") != Some("Spell")
                || part.get("Activator") != Some("You")
                || !same("ValidCard")
            {
                return unmapped("static:CostModifier-next-spell-shape");
            }
            let Some(times) = part.get("Amount").and_then(|text| text.parse::<u32>().ok()) else {
                return unmapped("key:Effect.ForgetOnCast-variable-amount");
            };
            let change = if mode == "ReduceCost" {
                "Reduce"
            } else {
                "Increase"
            };
            let mut cost = json!({"change": change, "mana": "{1}"});
            if times != 1 {
                cost["per"] = json!(times);
            }
            Ok(json!({"Cost": cost}))
        }
        "CastWithFlash" => {
            check_keys(
                part,
                "Effect.ForgetOnCast.flash",
                &["Mode", "ValidCard", "ValidSA", "Caster"],
                |_, _| false,
            )?;
            if part.get("ValidSA") != Some("Spell")
                || part.get("Caster") != Some("You")
                || !same("ValidCard")
            {
                return unmapped("static:CastWithFlash-next-spell-shape");
            }
            Ok(json!("Flash"))
        }
        "Continuous" if part.has("AddKeyword") => {
            check_keys(
                part,
                "Effect.ForgetOnCast.keyword",
                &["Mode", "Affected", "AffectedZone", "AddKeyword"],
                |_, _| false,
            )?;
            if part.get("AffectedZone") != Some("Stack") || !same("Affected") {
                return unmapped("static:Continuous-next-spell-shape");
            }
            let keyword = keyword_value(ctx, part.get("AddKeyword").unwrap_or_default())?;
            let helps = keyword == json!("Convoke")
                || keyword == json!("Improvise")
                || keyword.get("Affinity").is_some();
            if !helps {
                return unmapped(format!("key:Effect.ForgetOnCast-keyword={keyword}"));
            }
            Ok(json!({"Keyword": keyword}))
        }
        "Continuous" if part.get("MayPlayWithoutManaCost") == Some("True") => {
            check_keys(
                part,
                "Effect.ForgetOnCast.free",
                &[
                    "Mode",
                    "MayPlay",
                    "MayPlayWithoutManaCost",
                    "MayPlayDontGrantZonePermissions",
                    "Affected",
                    "ValidSA",
                    "AffectedZone",
                ],
                |_, _| false,
            )?;
            if part.get("MayPlay") != Some("True")
                || !matches!(part.get("ValidSA"), None | Some("Spell"))
                || !same("Affected")
            {
                return unmapped("static:MayPlay-next-spell-shape");
            }
            Ok(json!("Free"))
        }
        other => unmapped(format!("static:Effect.ForgetOnCast-{other}")),
    }
}
