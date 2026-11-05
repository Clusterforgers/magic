//! Effects that reveal a hand, name a card, copy a spell or choose a player.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, only, unmapped};
use super::valid::{defined, selector_any, subject};
use crate::forge::script::Params;

/// The effects of a verb this module reads; `None` for any other API.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Option<Tr<Vec<Value>>> {
    let node = match api {
        "RevealHand" => reveal_hand(ctx, p),
        "NameCard" => name_card(ctx, p),
        "CopySpellAbility" => copy_spell(ctx, p),
        "ChoosePlayer" => choose_player(ctx, p),
        _ => return None,
    };
    Some(node.map(|node| vec![node]))
}

/// `RevealHand`: the player shows every card of their hand. Looking at the hand (`Look$`) or remembering the cards
/// are other effects and stay gaps.
fn reveal_hand(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(p, "RevealHand", &["Defined", "ValidTgts"])?;
    let who = subject(ctx, p, "You", None)?;
    Ok(json!({"Reveal": {"what": {"Each": [{"OwnedBy": who}, {"In": "Hand"}]}}}))
}

/// `CopySpellAbility` of a spell: a target spell, or the spell a cast trigger is about. A copy of an ability, a copy
/// for each way to target, and a controller of the copy other than the controller of the effect stay gaps.
fn copy_spell(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "CopySpellAbility",
        &[
            "Defined",
            "ValidTgts",
            "TargetType",
            "MayChooseTarget",
            "Amount",
            "Controller",
            "SetColor",
        ],
    )?;
    if p.get("Controller").is_some_and(|who| who != "You") {
        return unmapped("key:CopySpellAbility.Controller");
    }
    let what = if p.has("ValidTgts") {
        if p.get("TargetType") != Some("Spell") {
            return unmapped("key:CopySpellAbility.TargetType");
        }
        let valid: Vec<&str> = p.get("ValidTgts").unwrap_or_default().split(',').collect();
        let mut selector = vec![json!("Spell")];
        if valid != ["Card"] {
            selector.extend(selector_any(ctx, &valid.join(","))?);
        }
        ctx.target(json!({"what": {"Object": selector}}))?
    } else if p.get("Defined") == Some("TriggeredSpellAbility")
        && ctx
            .trigger
            .as_ref()
            .is_some_and(|shape| shape.mode == "SpellCast")
    {
        json!("Triggering")
    } else {
        return unmapped("key:CopySpellAbility.Defined");
    };
    let mut copy = json!({"what": what});
    if p.has("Amount") {
        copy["count"] = amount(ctx, p.get("Amount").unwrap_or("1"))?;
    }
    if p.get("MayChooseTarget") == Some("True") {
        copy["new_targets"] = json!(true);
    }
    if let Some(color) = p.get("SetColor") {
        if !matches!(color, "White" | "Blue" | "Black" | "Red" | "Green") {
            return unmapped("key:CopySpellAbility.SetColor");
        }
        copy["except"] = json!([{"SetColors": [color]}]);
    }
    Ok(json!({"CopySpell": copy}))
}

/// `ChoosePlayer`: the chooser picks one of the opponents or of all players; the later lines read it as `ChosenPlayer`.
fn choose_player(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(p, "ChoosePlayer", &["Defined", "Choices", "ChoiceTitle"])?;
    let from = match p.get("Choices") {
        Some("Player.Opponent") => json!("Opponents"),
        Some("Player") => json!("EachPlayer"),
        _ => return unmapped("key:ChoosePlayer.Choices"),
    };
    let who = defined(ctx, p.get("Defined").unwrap_or("You"))?;
    let bind = ctx.fresh("player");
    ctx.chosen_card = Some(bind.clone());
    Ok(json!({"Choose": {"who": who, "from": from, "as": bind}}))
}

/// `NameCard`: the chooser names a card of the game that `ValidCards$` matches; the later lines read it as the chosen
/// name. A random name or a list to choose from is a gap.
fn name_card(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "NameCard",
        &["Defined", "ValidCards", "ValidDescription"],
    )?;
    let who = defined(ctx, p.get("Defined").unwrap_or("You"))?;
    let selector = match p.get("ValidCards") {
        None | Some("Card") => Vec::new(),
        Some(valid) => selector_any(ctx, valid)?,
    };
    let bind = ctx.fresh("name");
    ctx.chosen = Some(bind.clone());
    ctx.chosen_kind = None;
    Ok(json!({"ChooseValue": {"who": who, "kind": {"CardName": selector}, "as": bind}}))
}
