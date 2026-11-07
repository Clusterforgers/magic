//! `CopyPermanent` and the delayed clean-up (`AtEOT$`) of created tokens: copy exceptions are written into the copy
//! (CR 707.9); a keyword for a stated duration is a continuous effect on the new tokens.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{CARD_TYPES, Ctx, Tr, flag, only, unmapped};
use super::effects_obj::color_names;
use super::keyword::keyword_list;
use super::valid::{objects, subject};
use crate::forge::script::Params;

/// `CopyPermanent`: a token that is a copy of a permanent, with the exceptions its `Set*`/`Add*` words state.
pub fn copy_permanent(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "CopyPermanent",
        &[
            "Defined",
            "ValidTgts",
            "TargetMin",
            "TargetMax",
            "NumCopies",
            "TokenTapped",
            "SetPower",
            "SetToughness",
            "SetColor",
            "SetCreatureTypes",
            "AddTypes",
            "AddKeywords",
            "NonLegendary",
            "PumpKeywords",
            "PumpDuration",
            "Controller",
            "AtEOT",
            "RememberTokens",
            "Populate",
            "TokenAttacking",
            "AddTriggers",
            "AddAbilities",
            "AddStaticAbilities",
            "AddSVars",
        ],
    )?;
    let remember = flag(p, "RememberTokens")?;
    let mut effects = Vec::new();
    let of = if flag(p, "Populate")? {
        if p.has("Defined") || p.has("ValidTgts") {
            return unmapped("key:CopyPermanent.Populate-with-subject");
        }
        // Populate (CR 701.36): one creature token the player controls is chosen and copied.
        let chosen = ctx.fresh("populated");
        effects.push(json!({"Choose": {
            "from": {"Each": [{"Type": "Creature"}, "Token", {"ControlledBy": "You"}]}, "as": chosen}}));
        json!({ "Bound": chosen })
    } else {
        objects(subject(ctx, p, "", None)?)?
    };
    let count = amount(ctx, p.get("NumCopies").unwrap_or("1"))?;
    let mut except = exceptions(ctx, p)?;
    let temporary = temporary_keywords(ctx, p)?;
    if temporary.is_none() {
        for keyword in pumped(ctx, p)? {
            except.push(json!({ "AddKeyword": keyword }));
        }
    }
    except.extend(super::effects_copy::granted_exceptions(ctx, p)?);
    let mut spec = json!({"CopyOf": {"of": of}});
    if !except.is_empty() {
        spec["CopyOf"]["except"] = json!(except);
    }
    let mut create = json!({
        "token": spec, "count": count, "tapped": flag(p, "TokenTapped")?});
    if flag(p, "TokenAttacking")? {
        create["attacking"] = json!(true);
    }
    // Who owns and controls the copies: the controller by default.
    match p.get("Controller") {
        None | Some("You") => {}
        Some(word @ ("TriggeredCardController" | "TargetedController" | "TargetedPlayer")) => {
            create["who"] = super::valid::defined(ctx, word)?;
        }
        Some(other) => return unmapped(format!("key:CopyPermanent.Controller={other}")),
    }
    if temporary.is_none() && !p.has("AtEOT") && !remember {
        effects.push(json!({ "CreateToken": create }));
        return Ok(effects);
    }
    let name = ctx.fresh("copy");
    create["as"] = json!(name);
    if remember {
        ctx.remembered = Some(json!({ "Bound": name }));
    }
    let bound = json!({ "Bound": name });
    effects.push(json!({ "CreateToken": create }));
    if let Some(keywords) = temporary {
        let mods: Vec<Value> = keywords
            .into_iter()
            .map(|keyword| json!({ "AddKeyword": keyword }))
            .collect();
        effects.push(json!({"Continuous": {
            "affected": bound, "mods": mods, "duration": "UntilEndOfTurn"}}));
    }
    if let Some(word) = p.get("AtEOT") {
        effects.push(at_eot(word, &bound)?);
    }
    Ok(effects)
}

/// The keywords `PumpKeywords$` names.
fn pumped(ctx: &Ctx, p: &Params) -> Tr<Vec<Value>> {
    p.get("PumpKeywords")
        .map_or_else(|| Ok(Vec::new()), |text| keyword_list(ctx, text))
}

/// The keywords that last until end of turn only: `PumpKeywords$` with `PumpDuration$ EOT`.
fn temporary_keywords(ctx: &Ctx, p: &Params) -> Tr<Option<Vec<Value>>> {
    match (p.get("PumpKeywords"), p.get("PumpDuration")) {
        (_, None) => Ok(None),
        (None, Some(_)) => unmapped("key:CopyPermanent.PumpDuration-without-keywords"),
        (Some(_), Some("EOT")) => pumped(ctx, p).map(Some),
        (Some(_), Some(other)) => unmapped(format!("key:CopyPermanent.PumpDuration={other}")),
    }
}

/// The changes written into the copy.
fn exceptions(ctx: &Ctx, p: &Params) -> Tr<Vec<Value>> {
    let mut mods = Vec::new();
    match (p.get("SetPower"), p.get("SetToughness")) {
        (Some(power), Some(toughness)) => {
            let (power, toughness) = (amount(ctx, power)?, amount(ctx, toughness)?);
            mods.push(json!({"SetPT": {"power": power, "toughness": toughness}}));
        }
        (None, None) => {}
        _ => return unmapped("key:CopyPermanent.half-stats"),
    }
    if let Some(colors) = p.get("SetColor") {
        mods.push(json!({"SetColors": color_names(colors)?}));
    }
    if let Some(types) = p.get("SetCreatureTypes") {
        let names = types.split(',').map(str::trim).collect::<Vec<_>>();
        if let Some(bad) = names
            .iter()
            .find(|name| !ctx.world.subtypes.contains(**name))
        {
            return unmapped(format!("key:CopyPermanent.SetCreatureTypes={bad}"));
        }
        mods.push(json!({"SetCreatureTypes": names}));
    }
    if let Some(types) = p.get("AddTypes") {
        for word in types.split('&').map(str::trim) {
            mods.push(if CARD_TYPES.contains(&word) {
                json!({"AddType": word})
            } else if word == "Legendary" {
                json!({"AddSupertype": word})
            } else if ctx.world.subtypes.contains(word) {
                json!({"AddSubtype": word})
            } else {
                return unmapped(format!("key:CopyPermanent.AddTypes={word}"));
            });
        }
    }
    if flag(p, "NonLegendary")? {
        mods.push(json!({"RemoveSupertype": "Legendary"}));
    }
    if let Some(keywords) = p.get("AddKeywords") {
        for keyword in keyword_list(ctx, keywords)? {
            mods.push(json!({ "AddKeyword": keyword }));
        }
    }
    Ok(mods)
}

/// `AtEOT$` on created objects: a delayed trigger at the next end step (or end of combat) that sacrifices, exiles,
/// destroys or returns exactly the objects bound by the creating step. The `Your` forms wait for the player's own end
/// step. Other words stay gaps.
pub fn at_eot(word: &str, bound: &Value) -> Tr<Value> {
    let (step, whose, verb) = match word {
        "Sacrifice" | "SacrificeCtrl" => ("End", "EachPlayer", "Sacrifice"),
        "Exile" => ("End", "EachPlayer", "Exile"),
        "Destroy" => ("End", "EachPlayer", "Destroy"),
        "Hand" => ("End", "EachPlayer", "Hand"),
        "YourSacrifice" => ("End", "You", "Sacrifice"),
        "YourExile" => ("End", "You", "Exile"),
        "YourSacrificeUpkeep" => ("Upkeep", "You", "Sacrifice"),
        "ExileCombat" => ("EndCombat", "EachPlayer", "Exile"),
        "SacrificeCombat" => ("EndCombat", "EachPlayer", "Sacrifice"),
        other => return unmapped(format!("key:AtEOT={other}")),
    };
    let effect = match verb {
        "Sacrifice" => json!({"Sacrifice": {
            "who": {"ControllerOf": bound}, "filter": [{"Is": bound}],
            "count": {"Count": [{"Is": bound}]}}}),
        "Exile" => json!({"Exile": {"what": bound}}),
        "Destroy" => json!({"Destroy": {"what": bound}}),
        _ => json!({"Move": {"what": bound, "to": "Hand"}}),
    };
    Ok(json!({"DelayedTrigger": {
        "when": {"StepBegins": {"step": step, "whose": whose}}, "effects": [effect]}}))
}
