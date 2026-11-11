//! Effects that reveal a hand, name a card, copy a spell or choose a player.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, flag, only, unmapped};
use super::valid::{defined, selector_any, spell_selector, subject};
use crate::forge::script::Params;

/// The effects of a verb this module reads; `None` for any other API.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Option<Tr<Vec<Value>>> {
    let node = match api {
        "RevealHand" => return Some(reveal_hand(ctx, p)),
        "NameCard" => name_card(ctx, p),
        "CopySpellAbility" => copy_spell(ctx, p),
        "ChoosePlayer" => choose_player(ctx, p),
        _ => return None,
    };
    Some(node.map(|node| vec![node]))
}

/// `RevealHand`: the player shows every card of their hand. Looking at the hand (`Look$`) is another effect and stays a gap. Remembering the cards
/// is the same step that names them.
fn reveal_hand(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "RevealHand",
        &["Defined", "ValidTgts", "RememberRevealed", "Look"],
    )?;
    // With a target, `Defined$ Targeted` only repeats the target.
    if p.has("ValidTgts") && !matches!(p.get("Defined"), None | Some("Targeted")) {
        return unmapped("key:RevealHand.Defined");
    }
    let who = subject(ctx, p, "You", None)?;
    // Looking is private to the controller; revealing is public, so the two never mix.
    match p.get("Look") {
        None => {}
        Some("True") if p.get("RememberRevealed").is_none() => {
            return Ok(vec![json!({"LookAtHand": {"of": who}})]);
        }
        Some(_) => return unmapped("key:RevealHand.Look"),
    }
    let hand = json!({"Each": [{"OwnedBy": who}, {"In": "Hand"}]});
    if p.get("RememberRevealed") != Some("True") {
        return Ok(vec![json!({"Reveal": {"what": hand}})]);
    }
    // The cards are named as they are shown, so the lines after read that hand and not the one a later step leaves.
    let name = ctx.fresh("revealed");
    ctx.remembered = Some(json!({ "Bound": name }));
    Ok(vec![
        json!({"Bind": {"what": hand, "as": name}}),
        json!({"Reveal": {"what": {"Bound": name}}}),
    ])
}

/// `CopySpellAbility` of a spell: a target spell, or the spell a cast trigger is about. A copy of an ability, a copy
/// for each way to target, and a controller of the copy other than the one a trigger names stays a gap.
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
            "Optional",
            "UnlessAI",
            "NonLegendary",
            "CopyForEachCanTarget",
            "ChooseOnlyOne",
        ],
    )?;
    let who = match p.get("Controller") {
        None | Some("You") => None,
        // The player who cast the spell the trigger is about.
        Some(
            "TriggeredCardController" | "TriggeredActivator" | "TriggeredSpellAbilityController",
        ) if ctx
            .trigger
            .as_ref()
            .is_some_and(|shape| shape.mode == "SpellCast") =>
        {
            Some(json!("TriggeringPlayer"))
        }
        // "That permanent's controller copies this spell": the player the target points to.
        Some(word @ ("TargetedController" | "TargetedOrController")) => Some(defined(ctx, word)?),
        Some(_) => return unmapped("key:CopySpellAbility.Controller"),
    };
    let what = if p.has("ValidTgts") {
        if let Some(kinds) = ability_kinds(p.get("TargetType")) {
            return copy_target_ability(ctx, p, &kinds, who.is_some());
        }
        // Without a kind the script targets a spell, as Forge copies spells by default.
        if !matches!(p.get("TargetType"), None | Some("Spell")) {
            return unmapped("key:CopySpellAbility.TargetType");
        }
        let valid: Vec<&str> = p.get("ValidTgts").unwrap_or_default().split(',').collect();
        let mut selector = vec![json!("Spell")];
        if valid != ["Card"] {
            selector.extend(spell_selector(ctx, &valid.join(","))?);
        }
        ctx.target(json!({"what": {"Object": selector}}))?
    } else if p.get("Defined") == Some("TriggeredSpellAbility")
        && ctx
            .trigger
            .as_ref()
            .is_some_and(|shape| shape.mode == "SpellCast")
    {
        json!("Triggering")
    } else if p.get("Defined") == Some("TriggeredSpellAbility")
        && ctx
            .trigger
            .as_ref()
            .is_some_and(|shape| shape.mode == "AbilityCast")
    {
        return copy_activated(ctx, p, who.is_some());
    } else if p.get("Defined") == Some("Parent") && ctx.trigger.is_none() {
        json!("This")
    } else if p.get("Defined") == Some("Targeted") && !p.has("ValidTgts") {
        ctx.last_target()?
    } else {
        return unmapped("key:CopySpellAbility.Defined");
    };
    let mut copy = json!({"what": what});
    if let Some(who) = &who {
        copy["who"] = who.clone();
    }
    if p.has("Amount") {
        copy["count"] = amount(ctx, p.get("Amount").unwrap_or("1"))?;
    }
    // A copy of this very spell when its text names no target has none to change, and the card says nothing of them.
    let untargeted_self = what == json!("Triggering")
        && ctx.trigger.as_ref().is_some_and(|shape| shape.itself)
        && !ctx.oracle.to_lowercase().contains("target");
    if p.get("MayChooseTarget") == Some("True") && !untargeted_self {
        copy["new_targets"] = json!(true);
    }
    if p.get("NonLegendary") == Some("True") {
        copy["except"] = json!([{"RemoveSupertype": "Legendary"}]);
    }
    if let Some(each) = each_target(ctx, p)? {
        copy["each_target"] = each;
    }
    if let Some(color) = p.get("SetColor") {
        if !matches!(color, "White" | "Blue" | "Black" | "Red" | "Green") {
            return unmapped("key:CopySpellAbility.SetColor");
        }
        copy["except"] = json!([{"SetColors": [color]}]);
    }
    let copy = json!({"CopySpell": copy});
    if p.get("Optional") != Some("True") {
        return Ok(copy);
    }
    // "May copy": the player who controls the copy decides.
    let mut may = json!({"effects": [copy]});
    if let Some(who) = &who {
        may["who"] = who.clone();
    }
    Ok(json!({"May": may}))
}

/// `CopyForEachCanTarget$`: "copy that spell for each other creature it could target. Each copy targets a different one of
/// those creatures." The selector of the objects; `None` without the key.
fn each_target(ctx: &Ctx, p: &Params) -> Tr<Option<Value>> {
    let Some(valid) = p.get("CopyForEachCanTarget") else {
        if p.has("ChooseOnlyOne") {
            return unmapped("key:CopySpellAbility.ChooseOnlyOne");
        }
        return Ok(None);
    };
    if p.has("Amount") || p.has("MayChooseTarget") {
        return unmapped("key:CopySpellAbility.CopyForEachCanTarget-with-a-count");
    }
    // "The copy targets that token": only one is named, so only one is copied.
    if p.get("ChooseOnlyOne") == Some("True") && valid != "Permanent.IsRemembered" {
        return unmapped("key:CopySpellAbility.ChooseOnlyOne");
    }
    Ok(Some(json!(selector_any(ctx, valid)?)))
}

/// The ability kinds of `TargetType$ Activated.YouCtrl,Triggered.YouCtrl` ("target activated or triggered ability you
/// control"): each kind and whether it says the controller; `None` for a list with a spell or another word.
fn ability_kinds(word: Option<&str>) -> Option<Vec<(&'static str, bool)>> {
    word?
        .split(',')
        .map(|kind| match kind {
            "Activated" => Some(("Activated", false)),
            "Activated.YouCtrl" => Some(("Activated", true)),
            "Triggered" => Some(("Triggered", false)),
            "Triggered.YouCtrl" => Some(("Triggered", true)),
            _ => None,
        })
        .collect()
}

/// "Copy target activated or triggered ability you control": a target ability on the stack, judged by its source when
/// `ValidTgts$` names more than any card. Kinds that disagree on the controller, another controller of the copy and a
/// spell's words are gaps.
fn copy_target_ability(
    ctx: &mut Ctx,
    p: &Params,
    kinds: &[(&str, bool)],
    other_controller: bool,
) -> Tr<Value> {
    if other_controller {
        return unmapped("key:CopySpellAbility.Controller-of-an-ability");
    }
    for key in [
        "Defined",
        "NonLegendary",
        "SetColor",
        "Optional",
        "CopyForEachCanTarget",
        "ChooseOnlyOne",
    ] {
        if p.has(key) {
            return unmapped(format!("key:CopySpellAbility.{key}-of-a-target-ability"));
        }
    }
    let yours = kinds.iter().all(|(_, yours)| *yours);
    if !yours && kinds.iter().any(|(_, yours)| *yours) {
        return unmapped("key:CopySpellAbility.TargetType-mixed-controllers");
    }
    let mut selector = match kinds {
        [(kind, _)] => vec![json!({ "Ability": kind })],
        _ => {
            let any: Vec<Value> = kinds.iter().map(|(kind, _)| json!({ "Ability": kind })).collect();
            vec![json!({ "AnyOf": any })]
        }
    };
    if yours {
        selector.push(json!({"ControlledBy": "You"}));
    }
    match p.get("ValidTgts") {
        Some("Card,Emblem" | "Card") => {}
        // An emblem is a source no card selector names.
        Some(valid) if !valid.contains("Emblem") => selector.extend(selector_any(ctx, valid)?),
        _ => return unmapped("key:CopySpellAbility.ValidTgts-of-a-target-ability"),
    }
    let what = ctx.target(json!({"what": {"Object": selector}}))?;
    let mut copy = json!({ "what": what });
    if p.has("Amount") {
        copy["count"] = amount(ctx, p.get("Amount").unwrap_or("1"))?;
    }
    if p.get("MayChooseTarget") == Some("True") {
        copy["new_targets"] = json!(true);
    }
    Ok(json!({ "CopyAbility": copy }))
}

/// "Copy that ability" after an activation: the ability on the stack the activation put there (`Dealer`), never its source.
/// The copy is the trigger controller's; another controller, a spell's words and a may are gaps.
fn copy_activated(ctx: &mut Ctx, p: &Params, other_controller: bool) -> Tr<Value> {
    if other_controller {
        return unmapped("key:CopySpellAbility.Controller-of-an-ability");
    }
    for key in [
        "NonLegendary",
        "SetColor",
        "Optional",
        "ValidTgts",
        "CopyForEachCanTarget",
        "ChooseOnlyOne",
    ] {
        if p.has(key) {
            return unmapped(format!("key:CopySpellAbility.{key}-of-an-ability"));
        }
    }
    // Forge marks "copy that ability" optional where the card has no "may" (Illusionist's Bracers).
    if !ctx
        .oracle
        .to_lowercase()
        .replace("you may choose new targets", "")
        .contains("may")
    {
        return unmapped("key:CopySpellAbility.of-an-ability-without-may");
    }
    let mut copy = json!({"what": "Dealer"});
    if p.has("Amount") {
        copy["count"] = amount(ctx, p.get("Amount").unwrap_or("1"))?;
    }
    if p.get("MayChooseTarget") == Some("True") {
        copy["new_targets"] = json!(true);
    }
    Ok(json!({ "CopyAbility": copy }))
}

/// `ChoosePlayer`: the chooser picks one of the opponents or of all players; the later lines read it as `ChosenPlayer`.
fn choose_player(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "ChoosePlayer",
        &["Defined", "Choices", "ChoiceTitle", "Random"],
    )?;
    let from = match p.get("Choices") {
        Some("Player.Opponent" | "Opponent" | "Player.Other") => json!("Opponents"),
        Some("Player") | None => json!("EachPlayer"),
        _ => return unmapped("key:ChoosePlayer.Choices"),
    };
    let who = defined(ctx, p.get("Defined").unwrap_or("You"))?;
    let bind = ctx.fresh("player");
    ctx.chosen_card = Some(bind.clone());
    // "Choose an opponent at random": the game picks, so who would choose does not matter.
    if flag(p, "Random")? {
        return Ok(json!({"ChooseAtRandom": {"from": from, "as": bind}}));
    }
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
