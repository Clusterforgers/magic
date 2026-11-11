//! Unspent mana that stays: `Mode$ UnspentMana` ("You don't lose unspent red mana as steps and phases end.") and
//! `Event$ LoseMana` replaced by `ReplaceMana` ("If you would lose unspent mana, that mana becomes colorless instead.").

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The mana type a Forge letter or colour word names.
fn mana_type(word: &str) -> Option<&'static str> {
    Some(match word {
        "W" | "White" => "White",
        "U" | "Blue" => "Blue",
        "B" | "Black" => "Black",
        "R" | "Red" => "Red",
        "G" | "Green" => "Green",
        "C" | "Colorless" => "Colorless",
        _ => return None,
    })
}

/// `S:Mode$ UnspentMana`: the players keep unspent mana of a type (every type with no `ManaType$`, for every player
/// with no `ValidPlayer$`).
pub fn ep24_unspent_mana(p: &Params) -> Tr<Value> {
    check_keys(
        p,
        "Static.UnspentMana",
        &["Mode", "ValidPlayer", "ManaType"],
        |_, _| false,
    )?;
    let who = match p.get("ValidPlayer") {
        None => "EachPlayer",
        Some("You") => "You",
        Some(_) => return unmapped("key:Static.UnspentMana.ValidPlayer"),
    };
    let mut rule = json!({"KeepUnspentMana": {"who": who}});
    if let Some(word) = p.get("ManaType") {
        let Some(mana) = mana_type(word) else {
            return unmapped("key:Static.UnspentMana.ManaType");
        };
        rule["KeepUnspentMana"]["mana"] = json!(mana);
    }
    Ok(json!({"Rule": {"rule": rule}}))
}

/// `S:Mode$ ManaConvert` for every type: you may spend mana as though it were mana of any color (or type), for everything
/// or to cast spells of one kind you control (`ValidCard$ Creature.YouCtrl | ValidSA$ Spell`).
pub fn ep24_mana_convert(ctx: &Ctx, p: &Params) -> Tr<Value> {
    check_keys(
        p,
        "Static.ManaConvert",
        &[
            "Mode",
            "ValidPlayer",
            "ManaConversion",
            "ValidCard",
            "ValidSA",
        ],
        |_, _| false,
    )?;
    if p.get("ValidPlayer") != Some("You") {
        return unmapped("key:Static.ManaConvert.ValidPlayer");
    }
    let spend_as = match p.get("ManaConversion") {
        Some("AnyType->AnyColor") => "AnyColor",
        Some("AnyType->AnyType") => "AnyType",
        _ => return unmapped("key:Static.ManaConvert.ManaConversion"),
    };
    let mut rule = json!({"SpendManaAs": {"who": "You", "spend_as": spend_as}});
    match (p.get("ValidCard"), p.get("ValidSA")) {
        (None, None) => {}
        (Some(card), Some("Spell")) => {
            let Some(kind) = card
                .strip_suffix(".YouCtrl")
                .filter(|kind| !kind.contains([',', '.', '+']))
            else {
                return unmapped("key:Static.ManaConvert.ValidCard");
            };
            rule["SpendManaAs"]["spells"] = json!(selector_any(ctx, kind)?);
        }
        _ => return unmapped("key:Static.ManaConvert.ValidCard"),
    }
    Ok(json!({"Rule": {"rule": rule}}))
}

/// The objects of `ManaReflected | ReflectProperty$ Is` whose colours the mana may be: a valid-string over the
/// battlefield, `Defined.ValidGraveyard` over the graveyard, or the source itself (`Defined.Self`).
pub fn ep24_color_among(ctx: &Ctx, valid: &str) -> Tr<Value> {
    if valid == "Defined.Self" {
        return Ok(json!(["This"]));
    }
    if let Some(spec) = valid.strip_prefix("Defined.ValidGraveyard ") {
        let mut preds = selector_any(ctx, spec)?;
        preds.push(json!({"In": "Graveyard"}));
        return Ok(json!(preds));
    }
    // "Any of the exiled card's colors": the card an imprint remembers, or every card exiled with the source.
    if valid == "Defined.Imprinted" {
        let Some(imprinted) = super::memory::slot_ref_loose(ctx, super::memory::Slot::Imprinted)
        else {
            return unmapped("key:ManaReflected.Imprinted-without-a-change");
        };
        return Ok(json!([{"Is": imprinted}, {"In": "Exile"}]));
    }
    if valid == "Defined.ExiledWith" {
        return Ok(json!([{"ExiledWith": "This"}, {"In": "Exile"}]));
    }
    if valid.is_empty() || valid.starts_with("Defined.") {
        return unmapped("key:ManaReflected.Is-Defined");
    }
    Ok(json!(selector_any(ctx, valid)?))
}

/// `R:Event$ LoseMana` replaced by a `ReplaceMana` of one type: the mana you would lose becomes that type instead.
pub fn ep24_lose_mana(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    check_keys(
        p,
        "Replacement.LoseMana",
        &["Event", "ValidPlayer", "ReplacementResult", "ReplaceWith"],
        |key, value| key == "ActiveZones" && value == "Battlefield",
    )?;
    if p.get("ValidPlayer") != Some("You") || p.get("ReplacementResult") != Some("Replaced") {
        return unmapped("key:Replacement.LoseMana.shape");
    }
    let Some(name) = p.get("ReplaceWith") else {
        return unmapped("key:Replacement.LoseMana.shape");
    };
    let sub = ctx.sub(name)?;
    let into = match (sub.get("DB"), sub.get("ReplaceType")) {
        (Some("ReplaceMana"), Some(word)) if sub.iter().count() == 2 => mana_type(word),
        _ => None,
    };
    let Some(into) = into else {
        return unmapped("key:Replacement.LoseMana.ReplaceWith");
    };
    Ok(json!({"Rule": {"rule": {"UnspentManaBecomes": {"who": "You", "into": into}}}}))
}
